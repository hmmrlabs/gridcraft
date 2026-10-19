//! Explicit consent and memory-only creator authentication for encrypted uploads.
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use serde_json::Value;
use std::io::Read;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{cell::RefCell, rc::Rc};
const MAX_PROTECTED: usize = 16 * 1024 * 1024;

const API: &str = "https://gridlink-mcp.grid-viewer-deploy-tools.workers.dev/api";
enum Reply {
    Started(Value),
    Polled(Value),
    Uploaded(String),
}
pub struct ShareDialog {
    bytes: Vec<u8>,
    consent: bool,
    password: String,
    confirmation: String,
    session: String,
    verification_url: String,
    user_code: String,
    authenticated: bool,
    expires: f64,
    next_poll: f64,
    interval: f64,
    pending: bool,
    error: String,
    replies: Receiver<Result<Reply, String>>,
    sender: Sender<Result<Reply, String>>,
}
impl ShareDialog {
    pub fn new(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() > MAX_PROTECTED {
            return Err("Protected sharing supports workbooks up to 16 MiB. Download XLSX instead.".into());
        }
        let (sender, replies) = mpsc::channel();
        Ok(Self {
            bytes,
            consent: false,
            password: String::new(),
            confirmation: String::new(),
            session: String::new(),
            verification_url: String::new(),
            user_code: String::new(),
            authenticated: false,
            expires: 0.0,
            next_poll: 0.0,
            interval: 5.0,
            pending: false,
            error: String::new(),
            replies,
            sender,
        })
    }
    /// Returns (close dialog, newly created URL). Cancel never uploads workbook bytes.
    pub fn show(&mut self, ctx: &egui::Context) -> (bool, Option<String>) {
        let now = now_ms();
        let replies: Vec<_> = self.replies.try_iter().collect();
        for reply in replies {
            self.pending = false;
            match reply {
                Err(error) => self.error = error,
                Ok(Reply::Uploaded(url)) => return (true, Some(url)),
                Ok(Reply::Started(value)) => {
                    self.session = value["session"].as_str().unwrap_or("").to_owned();
                    self.verification_url = value["verificationUrl"].as_str().unwrap_or("").to_owned();
                    self.user_code = value["userCode"].as_str().unwrap_or("").to_owned();
                    self.interval = value["interval"].as_f64().unwrap_or(5.0).clamp(5.0, 60.0);
                    self.expires = now + value["expiresIn"].as_f64().unwrap_or(300.0).clamp(0.0, 600.0) * 1000.0;
                    self.next_poll = now + self.interval * 1000.0;
                    if self.session.len() != 64
                        || !self.session.bytes().all(|b| b.is_ascii_hexdigit())
                        || url::Url::parse(&self.verification_url)
                            .ok()
                            .is_none_or(|u| u.scheme() != "https" || !u.username().is_empty() || u.password().is_some())
                    {
                        self.session.clear();
                        self.verification_url.clear();
                        self.error = "Invalid sign-in response.".into();
                    }
                }
                Ok(Reply::Polled(value)) => {
                    if value["authenticated"].as_bool() == Some(true) {
                        self.authenticated = true;
                        self.expires = now + 3_500_000.0;
                        self.error.clear();
                    } else {
                        if value["slowDown"].as_bool() == Some(true) {
                            self.interval += 5.0;
                        }
                        self.next_poll = now + self.interval * 1000.0;
                        self.error = "Sign-in still pending. Finish in the WorkOS tab, then check again.".into();
                    }
                }
            }
        }
        if !self.session.is_empty() && now >= self.expires && !self.pending {
            self.session.clear();
            self.authenticated = false;
            self.error = "Sign-in expired. Start again.".into();
        }
        let mut close = false;
        let mut start = false;
        let mut poll = false;
        let mut upload = false;
        egui::Window::new("Share a protected workbook").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(format!("Workbook: {:.1} KiB · Link expires after 30 days", self.bytes.len() as f64 / 1024.0));
            ui.label("All sheets, including hidden sheets, will be included.");
            ui.label("GridCraft encrypts the workbook before uploading it to Cloudflare.");
            ui.add_enabled(!self.pending, egui::Checkbox::new(&mut self.consent, "I consent to storing this encrypted workbook for 30 days."));
            if self.consent {
                if self.session.is_empty() {
                    start = ui.add_enabled(!self.pending, egui::Button::new("Sign in / Create account")).clicked();
                } else if !self.authenticated {
                    ui.hyperlink_to("Continue with WorkOS", &self.verification_url);
                    ui.label(format!("Verification code: {}", self.user_code));
                    poll = ui.add_enabled(!self.pending && now >= self.next_poll, egui::Button::new("Check sign-in")).clicked();
                    ctx.request_repaint_after(std::time::Duration::from_secs(1));
                } else {
                    ui.label("Signed in. Choose a separate share password; send it separately from the link.");
                    ui.add_enabled(
                        !self.pending,
                        egui::TextEdit::singleline(&mut self.password).password(true).char_limit(256).hint_text("Share password (12+ characters)"),
                    );
                    ui.add_enabled(
                        !self.pending,
                        egui::TextEdit::singleline(&mut self.confirmation).password(true).char_limit(256).hint_text("Confirm share password"),
                    );
                    upload = ui.add_enabled(!self.pending, egui::Button::new("Encrypt and upload")).clicked();
                }
            }
            if !self.error.is_empty() {
                ui.label(&self.error);
            }
            if self.pending {
                ui.spinner();
                ui.label("Working…");
            }
            close = ui.add_enabled(!self.pending, egui::Button::new("Cancel")).clicked();
        });
        if start || poll {
            let token = self.session.clone();
            if poll {
                self.next_poll = now + self.interval * 1000.0;
            }
            self.launch(ctx, move || {
                if start { request("auth/start", None, b"").map(Reply::Started) } else { request("auth/poll", Some(&token), b"").map(Reply::Polled) }
            });
        }
        if upload && self.consent && self.authenticated {
            if self.password != self.confirmation || self.password.chars().count() < 12 || self.password.len() > 256 {
                self.error = "Passwords must match and contain at least 12 characters (at most 256 bytes).".into();
            } else {
                let password = std::mem::take(&mut self.password);
                self.confirmation.clear();
                let bytes = self.bytes.clone();
                let token = self.session.clone();
                self.launch(ctx, move || {
                    let ciphertext = encrypt(&bytes, &password)?;
                    let value = request("links", Some(&token), &ciphertext)?;
                    let id =
                        value["id"].as_str().filter(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())).ok_or("Invalid share ID.")?;
                    Ok(Reply::Uploaded(format!("https://grid-viewer.pages.dev/grid#s={id}")))
                });
            }
        }
        (close, None)
    }
    fn launch(&mut self, ctx: &egui::Context, job: impl FnOnce() -> Result<Reply, String> + Send + 'static) {
        self.pending = true;
        self.error.clear();
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        if std::thread::Builder::new()
            .name("protected-share".into())
            .spawn(move || {
                let _ = sender.send(job());
                ctx.request_repaint();
            })
            .is_err()
        {
            self.pending = false;
            self.error = "Could not start sharing worker. Try again.".into();
        }
    }
}
#[allow(clippy::disallowed_methods)] // Desktop-only module; the WASM viewer uses WebCrypto and its own clock.
fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs_f64() * 1000.0
}
fn request(path: &str, token: Option<&str>, bytes: &[u8]) -> Result<Value, String> {
    // Redirects are disabled so a bearer session cannot travel to another host.
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(30)).redirects(0).build();
    let mut request = agent.post(&format!("{API}/{path}")).set("Content-Type", "application/octet-stream");
    if let Some(token) = token {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    let response = request.send_bytes(bytes).map_err(|error| match error {
        ureq::Error::Status(401, _) => "Sign-in expired or not yet available. Cancel and sign in again.",
        ureq::Error::Status(429, _) => "Too many requests. Wait a minute and try again.",
        ureq::Error::Status(503, _) => "Account signup is not configured yet. Save a copy to keep your work.",
        _ => "Sharing request failed. Your workbook remains local; try again or save a copy.",
    })?;
    if response.status() != 200 {
        return Err("Unexpected sharing response.".into());
    }
    let mut body = Vec::new();
    response.into_reader().take(16_385).read_to_end(&mut body).map_err(|_| "Cannot read sharing response.")?;
    if body.len() > 16_384 {
        return Err("Sharing response too large.".into());
    }
    serde_json::from_slice(&body).map_err(|_| "Invalid sharing response.".into())
}
/// Same v2 wire format as Grid Viewer's browser WebCrypto implementation.
fn encrypt(bytes: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_PROTECTED {
        return Err("Protected sharing supports workbooks up to 16 MiB.".into());
    }
    if password.chars().count() < 12 || password.len() > 256 {
        return Err("Use a password of at least 12 characters and at most 256 bytes.".into());
    }
    let mut salt = [0_u8; 16];
    let mut nonce = [0_u8; 12];
    getrandom::getrandom(&mut salt).map_err(|_| "Secure random generator unavailable.")?;
    getrandom::getrandom(&mut nonce).map_err(|_| "Secure random generator unavailable.")?;
    let mut key = [0_u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2_10::Sha256>(password.as_bytes(), &salt, 600_000, &mut key);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "Invalid encryption key.")?;
    let encrypted = cipher.encrypt(Nonce::from_slice(&nonce), bytes).map_err(|_| "Encryption failed.")?;
    let mut blob = vec![2];
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce);
    blob.extend(encrypted);
    Ok(blob)
}

/// Desktop shell owns the private dialog state; none of it enters workbook/prefs/command logs.
pub struct Host {
    inbox: Rc<RefCell<Option<Vec<u8>>>>,
    dialog: Option<ShareDialog>,
    link: Option<String>,
}
impl Host {
    pub fn attach(app: &mut gridcraft_ui_egui::SheetApp) -> Self {
        let inbox = Rc::new(RefCell::new(None));
        let capture = inbox.clone();
        app.services.protected_share = Some(Box::new(move |bytes| *capture.borrow_mut() = Some(bytes)));
        Self { inbox, dialog: None, link: None }
    }
    pub fn ui(&mut self, ctx: &egui::Context, app: &mut gridcraft_ui_egui::SheetApp) {
        let incoming = self.inbox.borrow_mut().take();
        if let Some(bytes) = incoming {
            if self.dialog.is_some() {
                app.message = Some(("Sharing".into(), "Finish or cancel the existing sharing dialog first.".into()));
            } else {
                match ShareDialog::new(bytes) {
                    Ok(dialog) => self.dialog = Some(dialog),
                    Err(error) => app.message = Some(("Sharing".into(), error)),
                }
            }
        }
        if let Some(dialog) = &mut self.dialog {
            let (close, link) = dialog.show(ctx);
            if let Some(link) = link {
                self.link = Some(link);
            }
            if close {
                self.dialog = None;
            }
        }
        let mut close = false;
        if let Some(link) = &self.link {
            egui::Window::new("Protected link ready").collapsible(false).show(ctx, |ui| {
                ui.label("Expires in 30 days. Send the share password separately.");
                ui.add(egui::TextEdit::singleline(&mut link.as_str()).desired_width(450.0));
                if ui.button("Copy link").clicked() {
                    ctx.copy_text(link.clone());
                }
                if ui.button("Open link").clicked() {
                    ctx.open_url(egui::OpenUrl::new_tab(link));
                }
                close = ui.button("Close").clicked();
            });
        }
        if close {
            self.link = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn creator_dialog_requires_consent_before_signin() {
        use egui_kittest::kittest::Queryable as _;
        let dialog = ShareDialog::new(vec![0; 100_000]).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(900.0, 500.0)).build_ui_state(
            |ui, dialog: &mut ShareDialog| {
                dialog.show(ui.ctx());
            },
            dialog,
        );
        h.run_steps(3);
        assert!(h.query_by_label("Sign in / Create account").is_none());
        assert!(h.query_by_label("Encrypt and upload").is_none());
        h.get_by_label("I consent to storing this encrypted workbook for 30 days.").click();
        h.run_steps(3);
        assert!(h.query_by_label("Sign in / Create account").is_some());
        assert!(!h.state().pending);
        assert!(h.query_by_label("Encrypt and upload").is_none());
        if let Ok(path) = std::env::var("GRIDCRAFT_SHARE_SNAPSHOT") {
            h.render().unwrap().save(path).unwrap();
        }
    }

    #[test]
    fn native_envelope_matches_recipient_format_and_authenticates() {
        let password = "correct horse share password";
        let blob = encrypt(b"synthetic workbook", password).unwrap();
        assert_eq!(blob.first(), Some(&2));
        assert_eq!(blob.len(), b"synthetic workbook".len() + 45);
        let mut key = [0_u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2_10::Sha256>(password.as_bytes(), &blob[1..17], 600_000, &mut key);
        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        assert_eq!(cipher.decrypt(Nonce::from_slice(&blob[17..29]), &blob[29..]).unwrap(), b"synthetic workbook");
        let mut changed = blob.clone();
        changed[30] ^= 1;
        assert!(cipher.decrypt(Nonce::from_slice(&changed[17..29]), &changed[29..]).is_err());
        assert!(encrypt(b"test", "short").is_err());
        assert!(encrypt(&vec![0; MAX_PROTECTED + 1], password).is_err());
    }

    #[test]
    fn host_handoff_keeps_snapshot_local_until_user_submits() {
        let mut session = gridcraft_engine::Session::new();
        session.new_workbook();
        let mut app = gridcraft_ui_egui::SheetApp::new(session, Default::default());
        let host = Host::attach(&mut app);
        app.services.protected_share.as_ref().unwrap()(b"local snapshot".to_vec());
        assert_eq!(host.inbox.borrow().as_deref(), Some(b"local snapshot".as_slice()));
        assert!(host.dialog.is_none());
        assert!(host.link.is_none());
        let dialog = ShareDialog::new(b"local snapshot".to_vec()).unwrap();
        assert!(!dialog.consent);
        assert!(!dialog.pending);
        assert!(!dialog.authenticated);
        assert!(dialog.session.is_empty());
        assert!(dialog.replies.try_recv().is_err());
    }
}
