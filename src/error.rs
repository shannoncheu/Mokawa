#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("会话已过期，请重新登录。")]
    Unauthorized,
    #[error("PikPak 要求安全验证。")]
    VerificationRequired(Option<String>),
    #[error("PikPak 拒绝此操作；账号连接仍有效。请检查 PikPak 权限或安全验证提示。")]
    AccessDenied,
    #[error("请求过于频繁，请稍后重试。")]
    RateLimited,
    #[error("PikPak 请求失败（HTTP {0}）。")]
    Http(u16),
    #[error("PikPak 拒绝了此操作。")]
    Api,
    #[error("网络请求失败，请检查连接后重试。")]
    Network,
    #[error("PikPak 返回了无法识别的数据。")]
    Protocol,
    #[error("PikPak 响应格式异常（HTTP {status}，Content-Type: {content_type}）；响应正文未记录。")]
    ProtocolResponse { status: u16, content_type: String },
    #[error("此个人访问令牌不具备该操作所需权限。账号连接仍有效。")]
    UnsupportedPermission,
    #[error("输入无效：{0}")]
    Invalid(&'static str),
    #[error("操作已取消。")]
    Cancelled,
    #[error("请先确认此操作。")]
    ConfirmationRequired,
    #[error("无法访问 Windows 凭据管理器，请检查当前用户权限后重试。")]
    Credentials,
    #[error("没有保存的账号，请先完成 PikPak 授权。")]
    NoSession,
    #[error("保存的会话数据已损坏，请清除会话后重新授权。 ")]
    InvalidSession,
    #[error(
        "缺少获授权的 PikPak 客户端注册凭据。请仅配置 PikPak 官方为你签发的客户端 ID 和 Secret。"
    )]
    ClientRegistrationRequired,
    #[error("PikPak 授权尚未接入：Mokawa 的客户端注册与回调规则仍待确认，当前无法登录。")]
    AuthorizationPending,
    #[error("本地文件操作失败，请检查目录权限和剩余空间。")]
    Io,
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
impl From<reqwest::Error> for Error {
    fn from(_: reqwest::Error) -> Self {
        Self::Network
    }
}
