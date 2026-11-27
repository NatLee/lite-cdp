use crate::AccessError;
use std::env;
use url::Url;
#[derive(Clone)]
pub struct AccessConfig {
    pub(crate) origin: Url,
    pub(crate) github: Option<ProviderCredentials>,
    pub(crate) google: Option<ProviderCredentials>,
    pub(crate) mock_base: Option<Url>,
    pub(crate) dynamic_client_registration: bool,
}
#[derive(Clone)]
pub(crate) struct ProviderCredentials {
    pub id: String,
    pub secret: String,
}
impl AccessConfig {
    pub fn from_env(default_origin: &str) -> Result<Self, AccessError> {
        let origin = env::var("ACCESS_PUBLIC_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| default_origin.to_owned());
        let mut config = Self::for_origin(&origin)?;
        config.github = provider("GITHUB");
        config.google = provider("GOOGLE");
        config.dynamic_client_registration = match env::var("ACCESS_DYNAMIC_CLIENT_REGISTRATION")
            .ok()
            .as_deref()
        {
            None | Some("" | "false" | "0") => false,
            Some("true" | "1") => true,
            _ => {
                return Err(AccessError::bad(
                    "invalid_configuration",
                    "ACCESS_DYNAMIC_CLIENT_REGISTRATION must be true or false",
                ))
            }
        };
        #[cfg(feature = "test-support")]
        if let Ok(base) = env::var("ACCESS_TEST_MOCK_PROVIDER_URL") {
            if !base.trim().is_empty() {
                config = config.with_mock_providers(&base)?;
            }
        }
        Ok(config)
    }
    pub fn for_origin(value: &str) -> Result<Self, AccessError> {
        let origin = Url::parse(value)
            .map_err(|_| AccessError::bad("invalid_origin", "Invalid public origin"))?;
        let local = matches!(
            origin.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
        if origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.path() != "/"
            || !(origin.scheme() == "https" || origin.scheme() == "http" && local)
        {
            return Err(AccessError::bad(
                "invalid_origin",
                "Public URL must be an HTTPS origin or loopback HTTP origin",
            ));
        }
        Ok(Self {
            origin,
            github: None,
            google: None,
            mock_base: None,
            dynamic_client_registration: false,
        })
    }
    /// Public registration is opt-in; administrative registration is always available.
    pub fn with_dynamic_client_registration(mut self, enabled: bool) -> Self {
        self.dynamic_client_registration = enabled;
        self
    }
    #[cfg(feature = "test-support")]
    pub fn with_mock_providers(mut self, base: &str) -> Result<Self, AccessError> {
        let validated = Self::for_origin(base)?;
        if !matches!(
            validated.origin.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        ) {
            return Err(AccessError::bad("invalid_mock", "Mocks must use loopback"));
        }
        self.mock_base = Some(validated.origin);
        self.github = Some(ProviderCredentials {
            id: "mock-client".into(),
            secret: "mock-only-secret".into(),
        });
        self.google = self.github.clone();
        Ok(self)
    }
    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{}", self.origin.origin().ascii_serialization(), path)
    }
    pub fn secure_cookies(&self) -> bool {
        self.origin.scheme() == "https"
    }
}
fn provider(name: &str) -> Option<ProviderCredentials> {
    let id = env::var(format!("ACCESS_{name}_CLIENT_ID")).ok()?;
    let secret = env::var(format!("ACCESS_{name}_CLIENT_SECRET")).ok()?;
    (!id.trim().is_empty() && !secret.trim().is_empty())
        .then_some(ProviderCredentials { id, secret })
}
