use std::net::SocketAddr;
use std::process;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand, ValueEnum};
use wraith_core::auth::keys::KeySource;
use wraith_core::client::{ConnectOptions, TransportMode};
use wraith_core::transport::TcpTransport;
#[cfg(feature = "tls")]
use wraith_core::transport::TlsTransport;
#[cfg(feature = "iroh")]
use wraith_core::transport::IrohTransport;
use wraith_core::transport::Transport;

#[derive(Parser)]
#[command(name = "wraith", version, about = "Wraith SSH tunnel client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Connect to a wraith server and start a SOCKS5 proxy / port forwarding session")]
    Connect {
        #[arg(long, help = "TCP/TLS server address (required for tcp/tls transport)", env = "WRAITH_SERVER")]
        server: Option<String>,

        #[arg(long, help = "iroh endpoint ID, base58-encoded (required for iroh transport)")]
        peer: Option<String>,

        #[arg(long, value_enum, default_value = "tcp", help = "Transport mode")]
        transport: TransportModeArg,

        #[arg(long, help = "SSH private key path", env = "WRAITH_IDENTITY")]
        identity: Option<String>,

        #[arg(long, default_value = "127.0.0.1:1080", help = "SOCKS5 listen address")]
        socks5: String,

        #[arg(long, action = clap::ArgAction::Append, help = "Port forward spec (repeatable, e.g. 5432:db:5432)")]
        forward: Vec<String>,

        #[arg(long, action = clap::ArgAction::Append, help = "Remote port forward spec (repeatable)")]
        remote_forward: Vec<String>,

        #[arg(long, help = "Upstream proxy URL (socks5:// or http://)")]
        proxy: Option<String>,

        #[arg(long, help = "iroh relay URL")]
        iroh_relay: Option<String>,

        #[arg(long, help = "SNI hostname for TLS")]
        tls_server_name: Option<String>,

        #[arg(long, help = "Accept self-signed TLS certs")]
        insecure: bool,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum TransportModeArg {
    Tcp,
    Tls,
    Iroh,
}

impl From<TransportModeArg> for TransportMode {
    fn from(val: TransportModeArg) -> Self {
        match val {
            TransportModeArg::Tcp => TransportMode::Tcp,
            TransportModeArg::Tls => TransportMode::Tls,
            TransportModeArg::Iroh => TransportMode::Iroh,
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Connect {
            server,
            peer,
            transport,
            identity,
            socks5,
            forward,
            remote_forward,
            proxy,
            iroh_relay,
            tls_server_name,
            insecure,
        } => {
            let identity_val = identity
                .ok_or_else(|| anyhow!("--identity is required (or set WRAITH_IDENTITY env var)"))?;
            let key_source = KeySource::File(identity_val.into());

            let transport_mode: TransportMode = transport.into();

            if proxy.is_some() && matches!(transport_mode, TransportMode::Tcp) {
                eprintln!("warning: --proxy with --transport tcp is effectively a no-op (TCP transport is already a direct connection); use the SOCKS5 server instead");
            }

            let mut opts = ConnectOptions::new(key_source)
                .transport_mode(transport_mode.clone())
                .socks5_addr(&socks5);

            if let Some(ref s) = server {
                opts = opts.server(s);
            }
            if let Some(ref p) = peer {
                opts = opts.peer(p);
            }
            for fwd in &forward {
                opts = opts.forward(fwd);
            }
            for rfwd in &remote_forward {
                opts = opts.remote_forward(rfwd);
            }
            if let Some(ref p) = proxy {
                opts = opts.proxy(p);
            }
            if let Some(ref r) = iroh_relay {
                opts = opts.iroh_relay(r);
            }
            if let Some(ref n) = tls_server_name {
                opts = opts.tls_server_name(n);
            }
            if insecure {
                opts = opts.insecure(true);
            }

            opts.validate().map_err(|e| anyhow!("{e}"))?;

            match transport_mode {
                TransportMode::Tcp => {
                    let addr: SocketAddr = server
                        .as_deref()
                        .ok_or_else(|| anyhow!("--server is required for tcp transport"))?
                        .parse()
                        .map_err(|e| anyhow!("invalid server address: {e}"))?;
                    let t = Arc::new(TcpTransport::new(addr));
                    connect_and_run(opts, t).await
                }
                TransportMode::Tls => {
                    #[cfg(not(feature = "tls"))]
                    {
                        return Err(anyhow!("TLS transport is not available (wraith-core built without 'tls' feature)"));
                    }
                    #[cfg(feature = "tls")]
                    {
                        let addr: SocketAddr = server
                            .as_deref()
                            .ok_or_else(|| anyhow!("--server is required for tls transport"))?
                            .parse()
                            .map_err(|e| anyhow!("invalid server address: {e}"))?;
                        let mut t = TlsTransport::new(addr);
                        if let Some(ref n) = tls_server_name {
                            t = t.with_server_name(n);
                        }
                        t = t.with_insecure(insecure);
                        let t = Arc::new(t);
                        connect_and_run(opts, t).await
                    }
                }
                TransportMode::Iroh => {
                    #[cfg(not(feature = "iroh"))]
                    {
                        return Err(anyhow!("iroh transport is not available (wraith-core built without 'iroh' feature)"));
                    }
                    #[cfg(feature = "iroh")]
                    {
                        use iroh::{NodeId, RelayUrl};
                        let node_id_str = peer
                            .as_deref()
                            .ok_or_else(|| anyhow!("--peer is required for iroh transport"))?;
                        let node_id: NodeId = node_id_str
                            .parse()
                            .map_err(|e| anyhow!("invalid iroh peer endpoint ID: {e}"))?;
                        let relay_url: Option<RelayUrl> = match iroh_relay.as_deref() {
                            Some(u) => Some(
                                u.parse()
                                    .map_err(|e| anyhow!("invalid iroh relay URL: {e}"))?,
                            ),
                            None => None,
                        };
                        let proxy_url: Option<url::Url> = match proxy.as_deref() {
                            Some(u) => Some(
                                u.parse()
                                    .map_err(|e| anyhow!("invalid proxy URL: {e}"))?,
                            ),
                            None => None,
                        };
                        let t = Arc::new(
                            IrohTransport::new(node_id, relay_url, proxy_url)
                                .await
                                .map_err(|e| anyhow!("failed to create iroh transport: {e}"))?,
                        );
                        connect_and_run(opts, t).await
                    }
                }
            }
        }
    }
}

async fn connect_and_run<T: Transport>(opts: ConnectOptions, transport: Arc<T>) -> Result<()> {
    wraith_core::client::ClientSession::new(opts, transport)
        .await
        .map_err(|e| anyhow!("{e}"))?
        .run()
        .await
        .map_err(|e| anyhow!("{e}"))
}