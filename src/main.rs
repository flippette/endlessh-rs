use clap::Parser;
use eyre::Result;
use owo_colors::OwoColorize;
use rand::{distributions::Uniform, Rng};
use std::{
    net::{IpAddr, SocketAddr},
    process,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    select, signal,
    sync::{Notify, RwLock},
    task, time,
};
use tracing::info;

const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .compact()
        .with_target(false)
        .without_time()
        .init();

    let args = Args::parse();

    let socket = TcpListener::bind(SocketAddr::new(args.addr, args.port)).await?;

    let buf = {
        let mut tmp = [0u8; 256];
        tmp[255] = b'\n';
        tmp[254] = b'\r';
        Arc::new(RwLock::new(tmp))
    };

    let noti = Arc::new(Notify::new());
    let noti2 = Arc::clone(&noti);
    task::spawn(async move {
        signal::ctrl_c().await.unwrap();
        noti2.notify_one();
    });

    task::spawn(randomizer(Arc::clone(&buf)));

    info!("now listening on {}", socket.local_addr()?);

    loop {
        select! {
            Ok((conn, addr)) = socket.accept() => {
                task::spawn(handler(conn, addr, Arc::clone(&buf)));
            }

            () = noti.notified() => {
                process::exit(0);
            }
        }
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 22)]
    port: u16,

    #[arg(short, long, default_value = "127.0.0.1")]
    addr: IpAddr,
}

async fn handler(conn: TcpStream, addr: SocketAddr, buf: Arc<RwLock<[u8; 256]>>) -> Result<()> {
    let (mut conn, client_version) = {
        let mut tmp = String::new();
        let mut reader = BufReader::new(conn);
        reader.read_line(&mut tmp).await?;
        (reader.into_inner(), tmp)
    };
    info!(
        "{} {addr} {}",
        "conn".yellow().bold(),
        client_version.trim_end().blue().italic(),
    );

    loop {
        if let Ok(0) = conn.try_read(&mut [0]) {
            info!("{} {}", "drop".red().bold(), addr.strikethrough());
            return Ok(());
        }

        conn.write_all(&*buf.read().await).await?;
        info!("{} {addr}", "ping".green().bold());
        time::sleep(KEEPALIVE_INTERVAL).await;
    }
}

async fn randomizer(buf: Arc<RwLock<[u8; 256]>>) {
    let distr = Uniform::new_inclusive(32, 127);

    loop {
        {
            let mut buf = buf.write().await;
            buf[0] = b'x';
            buf[1..254]
                .iter_mut()
                .for_each(|ch| *ch = rand::thread_rng().sample(distr));
        }

        time::sleep(KEEPALIVE_INTERVAL).await;
    }
}
