use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::header::ContentType,
    transport::smtp::{authentication::Credentials, client::Tls},
};

#[derive(Clone)]
pub struct Mailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl Mailer {
    pub fn from_env() -> anyhow::Result<Self> {
        let host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".into());
        let port: u16 = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1025);
        let from = std::env::var("MAIL_FROM").unwrap_or_else(|_| "noreply@simu.local".into());

        let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host)
            .port(port)
            .tls(Tls::None);
        if let (Ok(user), Ok(pass)) = (std::env::var("SMTP_USER"), std::env::var("SMTP_PASSWORD")) {
            builder = builder.credentials(Credentials::new(user, pass));
        }
        let transport = builder.build();
        Ok(Self { transport, from })
    }

    pub async fn send_password_reset(&self, to: &str, reset_url: &str) -> anyhow::Result<()> {
        let body = format!(
            "Hello,\n\nClick this link to reset your password:\n\n  {reset_url}\n\nLink expires in 60 minutes.\n\nIf you didn't request this, ignore this email.\n\n— simu"
        );
        let email = Message::builder()
            .from(self.from.parse()?)
            .to(to.parse()?)
            .subject("Reset your simu password")
            .header(ContentType::TEXT_PLAIN)
            .body(body)?;
        self.transport.send(email).await?;
        Ok(())
    }

    pub async fn send_new_ip_login(&self, to: &str, ip: &str, ua: &str) -> anyhow::Result<()> {
        let body = format!(
            "A new sign-in to your simu account was just detected.\n\n  IP:         {ip}\n  Device/UA:  {ua}\n\nIf this wasn't you, change your password and revoke sessions immediately.\n\n— simu"
        );
        let email = Message::builder()
            .from(self.from.parse()?)
            .to(to.parse()?)
            .subject("New sign-in to your simu account")
            .header(ContentType::TEXT_PLAIN)
            .body(body)?;
        self.transport.send(email).await?;
        Ok(())
    }

    pub async fn send_email_verification(
        &self,
        to: &str,
        verify_url: &str,
    ) -> anyhow::Result<()> {
        let body = format!(
            "Welcome to simu!\n\nConfirm your email by clicking:\n\n  {verify_url}\n\nLink expires in 48 hours.\n\n— simu"
        );
        let email = Message::builder()
            .from(self.from.parse()?)
            .to(to.parse()?)
            .subject("Confirm your simu email")
            .header(ContentType::TEXT_PLAIN)
            .body(body)?;
        self.transport.send(email).await?;
        Ok(())
    }
}
