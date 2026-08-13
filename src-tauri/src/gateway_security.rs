use axum::http::{header, HeaderMap};
use uuid::Uuid;

const AUTHORIZATION_PREFIX: &str = "Bearer ";
const WEBSOCKET_PROTOCOL_PREFIX: &str = "codex-thread-studio.auth.";

#[derive(Clone)]
pub struct GatewaySecurity {
    expected_origin: String,
    expected_host: String,
    token: String,
    #[cfg(test)]
    disabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationError {
    InvalidOrigin,
    InvalidHost,
    MissingCredential,
    InvalidCredential,
}

impl GatewaySecurity {
    pub fn new(expected_origin: String) -> Self {
        let expected_host = expected_origin
            .strip_prefix("http://")
            .expect("Studio gateway origin uses HTTP")
            .to_string();
        Self {
            expected_origin,
            expected_host,
            token: Uuid::new_v4().simple().to_string(),
            #[cfg(test)]
            disabled: false,
        }
    }

    #[cfg(test)]
    pub fn for_tests(expected_origin: &str, token: &str) -> Self {
        Self {
            expected_origin: expected_origin.to_string(),
            expected_host: expected_origin
                .strip_prefix("http://")
                .expect("test origin uses HTTP")
                .to_string(),
            token: token.to_string(),
            disabled: false,
        }
    }

    #[cfg(test)]
    pub fn disabled_for_tests() -> Self {
        Self {
            expected_origin: "http://127.0.0.1:0".to_string(),
            expected_host: "127.0.0.1:0".to_string(),
            token: "tests-only-disabled-gateway-token".to_string(),
            disabled: true,
        }
    }

    pub fn authorize_http(&self, headers: &HeaderMap) -> Result<(), AuthorizationError> {
        #[cfg(test)]
        if self.disabled {
            return Ok(());
        }

        let Some(value) = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
        else {
            return Err(AuthorizationError::MissingCredential);
        };
        let Some(token) = value.strip_prefix(AUTHORIZATION_PREFIX) else {
            return Err(AuthorizationError::InvalidCredential);
        };
        if constant_time_eq(token.as_bytes(), self.token.as_bytes()) {
            self.validate_host(headers)?;
            self.validate_optional_origin(headers)
        } else {
            Err(AuthorizationError::InvalidCredential)
        }
    }

    pub fn authorize_websocket(&self, headers: &HeaderMap) -> Result<(), AuthorizationError> {
        #[cfg(test)]
        if self.disabled {
            return Ok(());
        }

        self.validate_required_origin(headers)?;
        let protocol = self.websocket_protocol();
        let Some(offered) = headers
            .get(header::SEC_WEBSOCKET_PROTOCOL)
            .and_then(|value| value.to_str().ok())
        else {
            return Err(AuthorizationError::MissingCredential);
        };
        if offered
            .split(',')
            .map(str::trim)
            .any(|candidate| constant_time_eq(candidate.as_bytes(), protocol.as_bytes()))
        {
            self.validate_host(headers)
        } else {
            Err(AuthorizationError::InvalidCredential)
        }
    }

    pub fn websocket_protocol(&self) -> String {
        format!("{WEBSOCKET_PROTOCOL_PREFIX}{}", self.token)
    }

    pub fn initialization_script(&self) -> String {
        let origin = serde_json::to_string(&self.expected_origin).expect("origin is JSON text");
        let token = serde_json::to_string(&self.token).expect("gateway token is JSON text");
        let host_platform =
            serde_json::to_string(std::env::consts::OS).expect("host platform is JSON text");
        format!(
            r#"if (window.location.origin === {origin}) {{
  Object.defineProperty(window, '__CODEX_THREAD_STUDIO_GATEWAY__', {{
    value: Object.freeze({{ token: {token}, hostPlatform: {host_platform} }}),
    configurable: false,
    enumerable: false,
    writable: false
  }});
}}"#
        )
    }

    fn validate_optional_origin(&self, headers: &HeaderMap) -> Result<(), AuthorizationError> {
        match headers.get(header::ORIGIN) {
            Some(origin) if origin.as_bytes() == self.expected_origin.as_bytes() => Ok(()),
            Some(_) => Err(AuthorizationError::InvalidOrigin),
            None => Ok(()),
        }
    }

    fn validate_host(&self, headers: &HeaderMap) -> Result<(), AuthorizationError> {
        match headers.get(header::HOST) {
            Some(host) if host.as_bytes() == self.expected_host.as_bytes() => Ok(()),
            _ => Err(AuthorizationError::InvalidHost),
        }
    }

    fn validate_required_origin(&self, headers: &HeaderMap) -> Result<(), AuthorizationError> {
        match headers.get(header::ORIGIN) {
            Some(origin) if origin.as_bytes() == self.expected_origin.as_bytes() => Ok(()),
            _ => Err(AuthorizationError::InvalidOrigin),
        }
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const ORIGIN: &str = "http://127.0.0.1:41721";
    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn authorizes_http_bearer_token_and_rejects_foreign_origins() {
        let security = GatewaySecurity::for_tests(ORIGIN, TOKEN);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer 0123456789abcdef0123456789abcdef"),
        );
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:41721"));
        assert_eq!(security.authorize_http(&headers), Ok(()));

        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://example.com"),
        );
        assert_eq!(
            security.authorize_http(&headers),
            Err(AuthorizationError::InvalidOrigin)
        );
    }

    #[test]
    fn websocket_requires_exact_origin_and_subprotocol() {
        let security = GatewaySecurity::for_tests(ORIGIN, TOKEN);
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, HeaderValue::from_static(ORIGIN));
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:41721"));
        headers.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("codex-thread-studio.auth.0123456789abcdef0123456789abcdef"),
        );
        assert_eq!(security.authorize_websocket(&headers), Ok(()));

        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://localhost:41721"),
        );
        assert_eq!(
            security.authorize_websocket(&headers),
            Err(AuthorizationError::InvalidOrigin)
        );
    }

    #[test]
    fn generated_initialization_script_is_origin_scoped() {
        let security = GatewaySecurity::for_tests(ORIGIN, TOKEN);
        let script = security.initialization_script();
        assert!(script.contains("window.location.origin === \"http://127.0.0.1:41721\""));
        assert!(script.contains(TOKEN));
        assert!(script.contains(&format!("hostPlatform: {:?}", std::env::consts::OS)));
        assert!(!script.contains("console"));
    }
}
