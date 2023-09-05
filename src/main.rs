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
    sync::{oneshot, RwLock},
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
        .init();

    let socket = TcpListener::bind(SocketAddr::new(
        rt_env!("ADDR", IpAddr::from([127, 0, 0, 1])),
        rt_env!("PORT", 22),
    ))
    .await?;

    let buf = {
        let mut tmp = [0u8; 256];
        tmp[255] = b'\n';
        tmp[254] = b'\r';
        Arc::new(RwLock::new(tmp))
    };

    let (tx, mut rx) = oneshot::channel();
    task::spawn(async move {
        signal::ctrl_c().await.unwrap();
        tx.send(()).unwrap();
    });

    task::spawn(randomizer(Arc::clone(&buf)));

    loop {
        select! {
            Ok((conn, addr)) = socket.accept() => {
                task::spawn(handler(conn, addr, Arc::clone(&buf)));
            }

            Ok(()) = &mut rx => {
                process::exit(0);
            }
        }
    }
}

async fn handler(
    conn: TcpStream,
    addr: SocketAddr,
    buf: Arc<RwLock<[u8; 256]>>,
) -> Result<()> {
    let (mut conn, client_version) = {
        let mut tmp = String::new();
        let mut reader = BufReader::new(conn);
        reader.read_line(&mut tmp).await?;
        (reader.into_inner(), tmp)
    };
    info!(
        "{} {addr} {}",
        "conn".yellow(),
        client_version.trim_end().blue()
    );

    loop {
        if let Ok(0) = conn.try_read(&mut [0]) {
            info!("{} {}", "drop".red(), addr.strikethrough());
            return Ok(());
        }

        conn.write_all(&*buf.read().await).await?;
        info!("{} {addr}", "ping".green());
        time::sleep(KEEPALIVE_INTERVAL).await;
    }
}

async fn randomizer(buf: Arc<RwLock<[u8; 256]>>) {
    let distr = Uniform::new_inclusive(32, 127);

    loop {
        let mut buf = buf.write().await;
        buf[0] = b'x';
        buf[1..254]
            .iter_mut()
            .for_each(|ch| *ch = rand::thread_rng().sample(distr));
        drop(buf); // unlock buf as soon as possible
        time::sleep(KEEPALIVE_INTERVAL).await;
    }
}

#[macro_export]
macro_rules! rt_env {
    ($var:expr, $dflt:expr) => {
        ::std::env::var($var)
            .ok()
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or($dflt)
    };
}
