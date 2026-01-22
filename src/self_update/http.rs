use crate::self_update::error::SelfUpdateError;
use std::sync::Arc;
use ureq::{Agent, AgentBuilder, native_tls};

pub struct HttpClient {
    agent: Agent,
}

impl HttpClient {
    pub fn new(allow_insecure: bool) -> Result<Self, SelfUpdateError> {
        if allow_insecure {
            let mut builder = native_tls::TlsConnector::builder();
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
            let connector = builder
                .build()
                .map_err(|error| SelfUpdateError::TlsConfig(error.to_string()))?;
            Ok(Self {
                agent: AgentBuilder::new()
                    .tls_connector(Arc::new(connector))
                    .build(),
            })
        } else {
            Ok(Self {
                agent: AgentBuilder::new().build(),
            })
        }
    }

    pub fn agent(&self) -> &Agent {
        &self.agent
    }
}
