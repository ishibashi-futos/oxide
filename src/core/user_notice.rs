use std::time::{Duration, Instant};

pub const DEFAULT_NOTICE_TTL_MS: u64 = 4_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserNoticeLevel {
    Info,
    Success,
    Warn,
    Error,
}

impl UserNoticeLevel {
    pub fn icon(self) -> &'static str {
        match self {
            UserNoticeLevel::Success => "✅",
            UserNoticeLevel::Info => "ℹ️",
            UserNoticeLevel::Warn => "⚠️",
            UserNoticeLevel::Error => "❌",
        }
    }

    fn priority(self) -> u8 {
        match self {
            UserNoticeLevel::Error => 3,
            UserNoticeLevel::Warn => 2,
            UserNoticeLevel::Success | UserNoticeLevel::Info => 1,
        }
    }

    fn can_replace(self, current: Self) -> bool {
        self.priority() >= current.priority()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserNotice {
    pub level: UserNoticeLevel,
    pub text: String,
    pub source: String,
    pub ttl_ms: Option<u64>,
}

impl UserNotice {
    pub fn new(level: UserNoticeLevel, text: impl Into<String>, source: impl Into<String>) -> Self {
        let ttl_ms = match level {
            UserNoticeLevel::Info | UserNoticeLevel::Success => Some(DEFAULT_NOTICE_TTL_MS),
            UserNoticeLevel::Warn | UserNoticeLevel::Error => None,
        };
        Self {
            level,
            text: text.into(),
            source: source.into(),
            ttl_ms,
        }
    }

    pub fn with_ttl_ms(
        level: UserNoticeLevel,
        text: impl Into<String>,
        source: impl Into<String>,
        ttl_ms: Option<u64>,
    ) -> Self {
        Self {
            level,
            text: text.into(),
            source: source.into(),
            ttl_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UserNoticeState {
    notice: UserNotice,
    expires_at: Option<Instant>,
}

impl UserNoticeState {
    fn from_notice(notice: UserNotice, now: Instant) -> Self {
        let expires_at = notice.ttl_ms.map(|ttl| now + Duration::from_millis(ttl));
        Self { notice, expires_at }
    }

    fn is_expired_at(&self, now: Instant) -> bool {
        match self.expires_at {
            Some(expires_at) => now >= expires_at,
            None => false,
        }
    }
}

#[derive(Debug, Default)]
pub struct UserNoticeQueue {
    current: Option<UserNoticeState>,
}

impl UserNoticeQueue {
    pub fn new() -> Self {
        Self { current: None }
    }

    pub fn current(&mut self, now: Instant) -> Option<&UserNotice> {
        let expired = match self.current.as_ref() {
            Some(state) => state.is_expired_at(now),
            None => false,
        };
        if expired {
            self.current = None;
        }
        self.current.as_ref().map(|state| &state.notice)
    }

    pub fn push(&mut self, notice: UserNotice, now: Instant) -> bool {
        let incoming = UserNoticeState::from_notice(notice, now);
        let expired = match self.current.as_ref() {
            Some(state) => state.is_expired_at(now),
            None => false,
        };
        if expired {
            self.current = None;
        }
        let should_replace = match self.current.as_ref() {
            None => true,
            Some(state) => incoming.notice.level.can_replace(state.notice.level),
        };
        if should_replace {
            self.current = Some(incoming);
        }
        should_replace
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_icons_match_spec() {
        assert_eq!(UserNoticeLevel::Success.icon(), "✅");
        assert_eq!(UserNoticeLevel::Info.icon(), "ℹ️");
        assert_eq!(UserNoticeLevel::Warn.icon(), "⚠️");
        assert_eq!(UserNoticeLevel::Error.icon(), "❌");
    }

    #[test]
    fn level_priority_respects_spec() {
        assert!(UserNoticeLevel::Error.can_replace(UserNoticeLevel::Warn));
        assert!(UserNoticeLevel::Warn.can_replace(UserNoticeLevel::Success));
        assert!(!UserNoticeLevel::Success.can_replace(UserNoticeLevel::Warn));
        assert!(UserNoticeLevel::Info.can_replace(UserNoticeLevel::Success));
    }

    #[test]
    fn notice_defaults_ttl_for_info_and_success() {
        let info = UserNotice::new(UserNoticeLevel::Info, "info", "test");
        let success = UserNotice::new(UserNoticeLevel::Success, "ok", "test");
        let warn = UserNotice::new(UserNoticeLevel::Warn, "warn", "test");
        let error = UserNotice::new(UserNoticeLevel::Error, "err", "test");

        assert_eq!(info.ttl_ms, Some(DEFAULT_NOTICE_TTL_MS));
        assert_eq!(success.ttl_ms, Some(DEFAULT_NOTICE_TTL_MS));
        assert_eq!(warn.ttl_ms, None);
        assert_eq!(error.ttl_ms, None);
    }

    #[test]
    fn notice_state_expires_when_ttl_passed() {
        let now = Instant::now();
        let notice = UserNotice::with_ttl_ms(UserNoticeLevel::Info, "info", "test", Some(10));
        let state = UserNoticeState::from_notice(notice, now);

        assert!(!state.is_expired_at(now));
        assert!(state.is_expired_at(now + Duration::from_millis(10)));
    }

    #[test]
    fn notice_state_is_persistent_without_ttl() {
        let now = Instant::now();
        let notice = UserNotice::with_ttl_ms(UserNoticeLevel::Warn, "warn", "test", None);
        let state = UserNoticeState::from_notice(notice, now);

        assert!(!state.is_expired_at(now + Duration::from_secs(10)));
    }

    #[test]
    fn queue_rejects_lower_priority_notice() {
        let now = Instant::now();
        let mut queue = UserNoticeQueue::new();
        queue.push(UserNotice::new(UserNoticeLevel::Error, "err", "test"), now);

        let replaced = queue.push(UserNotice::new(UserNoticeLevel::Info, "info", "test"), now);

        assert!(!replaced);
        assert_eq!(queue.current(now).unwrap().text, "err");
    }

    #[test]
    fn queue_accepts_equal_priority_notice() {
        let now = Instant::now();
        let mut queue = UserNoticeQueue::new();
        queue.push(UserNotice::new(UserNoticeLevel::Warn, "warn1", "test"), now);

        let replaced = queue.push(UserNotice::new(UserNoticeLevel::Warn, "warn2", "test"), now);

        assert!(replaced);
        assert_eq!(queue.current(now).unwrap().text, "warn2");
    }

    #[test]
    fn queue_drops_expired_notice_on_read() {
        let now = Instant::now();
        let mut queue = UserNoticeQueue::new();
        queue.push(
            UserNotice::with_ttl_ms(UserNoticeLevel::Info, "info", "test", Some(5)),
            now,
        );

        assert!(queue.current(now + Duration::from_millis(10)).is_none());
    }
}
