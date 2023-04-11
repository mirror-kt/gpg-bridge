use std::ffi::c_void;
use std::io::{self, Error};
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use log::{debug, error, trace};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::sync::{Semaphore, SemaphorePermit};
use windows::core::HSTRING;
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    MEMORYMAPPEDVIEW_HANDLE, PAGE_READWRITE,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SendMessageW, WM_COPYDATA};

use crate::listener::Listener;
use crate::ping_gpg_agent;
use crate::stream::SplitStream;
use crate::util::other_error;

// For now, forwarding ssh agent requests can only be done using IPC messages. gpg
// ssh agent seems to do security trick on tcp stream and fail to receive anything.
async fn delegate_ssh(mut from: impl SplitStream) -> io::Result<()> {
    let (mut source_read, mut source_write) = from.split_rw();
    let mut handler = Handler::new().await?;
    while let Some(resp) = handler.process_one(&mut source_read).await? {
        trace!("get {:?}", String::from_utf8_lossy(resp));
        source_write.write_all(resp).await?;
    }
    debug!(
        "connection finished, received {}, replied {}",
        handler.received(),
        handler.replied()
    );
    Ok(())
}

pub async fn bridge_to_message<L>(mut listener: L) -> io::Result<()>
where
    L: Listener,
    L::Connection: SplitStream + Send + 'static,
{
    let reload = Arc::new(AtomicBool::new(false));
    loop {
        let conn = listener.accept().await?;

        if reload.load(Ordering::SeqCst) {
            ping_gpg_agent().await?;
            reload.store(false, Ordering::SeqCst);
        }
        let reload = reload.clone();
        tokio::spawn(async move {
            if let Err(e) = delegate_ssh(conn).await {
                error!("failed to delegate message: {:?}", e);
                reload.store(true, Ordering::SeqCst);
            }
        });
    }
}

/// A magic value used with WM_COPYDATA.
const PUTTY_IPC_MAGIC: usize = 0x804e50ba;
static FILE_MAP_NAME: &str = "gpg_bridge";
static PAGEANT_WINDOW_NAME: &str = "Pageant";

/// To avoid surprises we limit the size of the mapped IPC file to this
/// value.  Putty currently (0.62) uses 8k, thus 16k should be enough
/// for the foreseeable future.  */
pub const PUTTY_IPC_MAXLEN: usize = 16384;

static CONCURRENCY: Semaphore = Semaphore::const_new(4);
static TOKEN: parking_lot::Mutex<u8> = parking_lot::const_mutex(0);

fn find_available_token() -> u8 {
    let mut token = TOKEN.lock();
    let mut mask = 1;
    for _ in 0..4 {
        if *token & mask == 0 {
            *token |= mask;
            return mask;
        }
        mask <<= 1;
    }
    unreachable!()
}

fn release_token(mask: u8) {
    let mut token = TOKEN.lock();
    *token &= !mask;
}

pub struct Handler {
    handle: HANDLE,
    view: *mut u8,
    limit: usize,
    mask: u8,
    name: String,
    _permit: SemaphorePermit<'static>,
    received: usize,
    replied: usize,
}

unsafe impl Send for Handler {}

impl Handler {
    pub async fn new() -> io::Result<Self> {
        let permit = CONCURRENCY.acquire().await.unwrap();
        let mask = find_available_token();
        let name = format!("{}-{}\0", FILE_MAP_NAME, mask);
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                PUTTY_IPC_MAXLEN as u32,
                &HSTRING::from(name.as_str()),
            )?
        };
        if handle.is_invalid() {
            release_token(mask);
            return Err(other_error(format!(
                "failed to create memory mapping: {}",
                Error::last_os_error()
            )));
        }

        let view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, PUTTY_IPC_MAXLEN)? };
        if view.is_invalid() {
            unsafe {
                CloseHandle(handle);
            }
            release_token(mask);
            return Err(other_error(format!(
                "can't map view of memory: {}",
                Error::last_os_error()
            )));
        }

        Ok(Handler {
            handle,
            view: view.0 as *mut u8,
            limit: PUTTY_IPC_MAXLEN,
            mask,
            name,
            _permit: permit,
            received: 0,
            replied: 0,
        })
    }

    pub async fn process_one(
        &mut self,
        reader: &mut Pin<Box<dyn AsyncRead + Send + '_>>,
    ) -> io::Result<Option<&[u8]>> {
        let len_bytes = unsafe { std::slice::from_raw_parts_mut(self.view, 4) };
        if let Err(e) = reader.read_exact(len_bytes).await {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(e);
        }
        let len = u32::from_be(unsafe { (self.view as *mut u32).read_unaligned() }) as usize + 4;
        if len > self.limit {
            return Err(other_error(format!(
                "message too large: {} >= {}",
                len, self.limit
            )));
        }
        self.received += len;
        let req = unsafe { std::slice::from_raw_parts_mut(self.view.add(4), len - 4) };
        reader.read_exact(req).await?;
        trace!("recv request: {:?}", String::from_utf8_lossy(req));
        let win = unsafe {
            FindWindowW(
                &HSTRING::from(PAGEANT_WINDOW_NAME),
                &HSTRING::from(PAGEANT_WINDOW_NAME),
            )
        };
        if win == HWND(0) {
            return Err(other_error(format!(
                "can't contact gpg agent: {}",
                Error::last_os_error()
            )));
        }
        let copy_data = COPYDATASTRUCT {
            dwData: PUTTY_IPC_MAGIC,
            cbData: self.name.len() as u32,
            lpData: self.name.as_mut_ptr() as *mut c_void,
        };
        let res = unsafe {
            SendMessageW(
                win,
                WM_COPYDATA,
                WPARAM::default(),
                LPARAM((&copy_data) as *const _ as _),
            )
        };
        if res == LRESULT(0) {
            return Err(other_error(format!(
                "failed to send message to pageant: {}",
                Error::last_os_error()
            )));
        }
        let len = u32::from_be(unsafe { (self.view as *mut u32).read_unaligned() }) as usize + 4;
        if len > self.limit {
            return Err(other_error(format!(
                "response too large: {} > {}",
                len + 4,
                self.limit
            )));
        };

        self.replied += len;
        unsafe { Ok(Some(std::slice::from_raw_parts(self.view, len))) }
    }

    pub fn received(&self) -> usize {
        self.received
    }

    pub fn replied(&self) -> usize {
        self.replied
    }
}

impl Drop for Handler {
    fn drop(&mut self) {
        unsafe {
            ptr::write_bytes(self.view, 0, self.limit);
            if !self.view.is_null() {
                UnmapViewOfFile(MEMORYMAPPEDVIEW_HANDLE(self.view as isize));
            }
            CloseHandle(self.handle);
        }
        release_token(self.mask);
    }
}
