mod common;
mod core;
mod middlewares;
mod modules;

fn main() {
    common::sudo::check_root();
    common::env::load();
    common::log::init();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            core::bootstrap::init().await;
        });
}
