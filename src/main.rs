mod db;
mod engine;
mod icecast;
mod listen;
mod module;
mod persist;
mod project;
mod rtmp;
mod server;
mod source;
mod throttle;
mod util;
mod video;

use structopt::StructOpt;

#[derive(StructOpt)]
struct Opts {
    #[structopt(flatten)]
    run: server::RunOpts,
}

fn main() {
    env_logger::init();

    let opts = Opts::from_args();

    let mut runtime = tokio::runtime::Builder::new()
        .enable_all()
        .threaded_scheduler()
        .build()
        .unwrap();

    runtime.block_on(server::run(opts.run));
}
