use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(
        long,
        value_name = "ADDRESS",
        required_unless_present = "extra",
        help = "Sets the listenning address to bridge the ssh socket"
    )]
    pub ssh: Option<String>,
    #[arg(
        long,
        value_name = "ADDRESS",
        required_unless_present = "ssh",
        help = "Sets the listenning address to bridge the extra socket"
    )]
    pub extra: Option<String>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Sets the path to gnupg extra socket optionaly"
    )]
    pub extra_socket: Option<String>,
    #[arg(short, long, help = "Runs the program as a background daemon")]
    pub detach: bool,
}
