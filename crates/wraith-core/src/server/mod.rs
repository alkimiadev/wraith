pub mod channel_proxy;
pub mod control_channel;
pub mod handler;
pub mod rate_limit;
pub mod serve;
pub mod stealth;

pub use channel_proxy::{connect_outbound, proxy_channel};
pub use control_channel::{
    ControlChannelHandler, ControlChannelRouter, DuplexStream, WRAITH_CONTROL_DESTINATION,
    WRAITH_PREFIX, is_reserved_destination,
};
pub use handler::{ProxyConfig, ProxyMode, ServerHandler, TransportKind};
pub use rate_limit::{AuthAttemptLimiter, ConnectionRateLimiter};
pub use serve::{Server, ServeError, ServeOptions, ServeTransportMode};
pub use stealth::{ProtocolDetection, detect_protocol, send_fake_nginx_404, validate_stealth_config};