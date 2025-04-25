use url::Url;

pub mod create;

pub struct PendingActivity<T> {
    pub activity: T,
    pub target_inboxes: Vec<Url>,
}