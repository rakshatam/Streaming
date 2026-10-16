use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::{error, info, warn};
use crate::config::Config;

/// Sends a security OTP verification email to the user.
/// Returns Ok(true) if email was successfully dispatched over SMTP.
/// Returns Ok(false) if SMTP is not configured (allowing graceful local fallback).
/// Returns Err(String) if SMTP connection was configured but sending failed.
pub async fn send_otp_email(
    to_email: &str,
    username: &str,
    otp_code: &str,
    config: &Config,
) -> Result<bool, String> {
    let smtp_host = match &config.smtp_host {
        Some(host) if !host.trim().is_empty() => host.clone(),
        _ => {
            warn!(
                "[EMAIL DISPATCHER] SMTP_HOST is not configured. To send live emails to inboxes, set SMTP_HOST, SMTP_PORT, SMTP_USER, SMTP_PASS in backend/.env"
            );
            return Ok(false);
        }
    };

    let from_mailbox: Mailbox = config
        .smtp_from
        .parse()
        .unwrap_or_else(|_| "StreamFlow <noreply@streamflow.tv>".parse().unwrap());

    let to_mailbox: Mailbox = format!("{} <{}>", username, to_email)
        .parse()
        .map_err(|e| format!("Invalid recipient email address: {}", e))?;

    let html_content = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>StreamFlow Security Verification</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background-color: #0b0f17; color: #f1f5f9; margin: 0; padding: 40px 20px; }}
    .container {{ max-width: 520px; margin: 0 auto; background-color: #131b2e; border: 1px solid #1e293b; border-radius: 16px; overflow: hidden; box-shadow: 0 20px 40px rgba(0,0,0,0.5); }}
    .header {{ background: linear-gradient(135deg, #e50914 0%, #b81d24 100%); padding: 28px; text-align: center; }}
    .header h1 {{ margin: 0; font-size: 24px; font-weight: 800; letter-spacing: -0.5px; color: #ffffff; text-transform: uppercase; }}
    .content {{ padding: 32px 28px; text-align: center; }}
    .welcome {{ font-size: 16px; color: #94a3b8; margin-bottom: 24px; }}
    .otp-box {{ background: #0b0f17; border: 2px dashed #e50914; border-radius: 12px; padding: 20px; margin: 24px 0; }}
    .otp-code {{ font-size: 38px; font-weight: 800; letter-spacing: 8px; color: #ffffff; font-family: 'SF Mono', Monaco, Consolas, monospace; }}
    .expiry {{ font-size: 13px; color: #ef4444; margin-top: 8px; font-weight: 600; }}
    .footer {{ padding: 20px 28px; background-color: #0a0e17; border-top: 1px solid #1e293b; text-align: center; font-size: 12px; color: #64748b; line-height: 1.6; }}
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>StreamFlow Security</h1>
    </div>
    <div class="content">
      <p class="welcome">Hello <strong style="color:#ffffff;">{}</strong>,</p>
      <p style="color:#cbd5e1; font-size:14px; line-height:1.6;">
        You recently initiated registration on <strong>StreamFlow</strong>. To verify your email address and activate your account, use the one-time verification code below:
      </p>
      <div class="otp-box">
        <div class="otp-code">{}</div>
        <div class="expiry">Expires in 10 minutes (Single Use)</div>
      </div>
      <p style="font-size: 13px; color: #94a3b8; line-height: 1.5;">
        If you did not request this verification code, you can safely ignore this email. Your account will not be created without this verification step.
      </p>
    </div>
    <div class="footer">
      Protected with HMAC-SHA256 and Argon2id Cryptographic Security.<br/>
      &copy; 2026 StreamFlow Inc. High-Throughput Streaming Platform.
    </div>
  </div>
</body>
</html>"#,
        username, otp_code
    );

    let email_msg = Message::builder()
        .from(from_mailbox)
        .to(to_mailbox)
        .subject(format!("StreamFlow: Your Verification Code is {}", otp_code))
        .header(ContentType::TEXT_HTML)
        .body(html_content)
        .map_err(|e| format!("Failed to build email message: {}", e))?;

    let mut transport_builder = if config.smtp_port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)
            .map_err(|e| format!("SMTP relay initialization failed: {}", e))?
            .port(config.smtp_port)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host)
            .map_err(|e| format!("SMTP starttls initialization failed: {}", e))?
            .port(config.smtp_port)
    };

    if let (Some(user), Some(pass)) = (&config.smtp_user, &config.smtp_pass) {
        transport_builder = transport_builder.credentials(Credentials::new(user.clone(), pass.clone()));
    }

    let transport = transport_builder.build();

    info!(
        "[EMAIL DISPATCHER] Contacting SMTP host {}:{} for recipient {}...",
        smtp_host, config.smtp_port, to_email
    );

    match transport.send(email_msg).await {
        Ok(resp) => {
            info!(
                "[EMAIL DISPATCHER] Successfully delivered OTP email to {} via SMTP (Response: {:?})",
                to_email, resp
            );
            Ok(true)
        }
        Err(e) => {
            error!(
                "[EMAIL DISPATCHER] SMTP transmission failed for {}: {}",
                to_email, e
            );
            Err(e.to_string())
        }
    }
}
