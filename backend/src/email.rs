use crate::config::Config;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::{error, info, warn};

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

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
    if otp_code.len() != 6 || !otp_code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("OTP code must contain exactly six digits".into());
    }

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
        .map_err(|error| format!("Invalid SMTP_FROM address: {error}"))?;

    let to_mailbox: Mailbox = format!("{} <{}>", username, to_email)
        .parse()
        .map_err(|e| format!("Invalid recipient email address: {}", e))?;

    let safe_username = escape_html(username);
    let safe_otp_code = escape_html(otp_code);
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
      StreamFlow Video Platform<br/>
      &copy; 2026 StreamFlow
    </div>
  </div>
</body>
</html>"#,
        safe_username, safe_otp_code
    );

    let email_msg = Message::builder()
        .from(from_mailbox)
        .to(to_mailbox)
        .subject("StreamFlow: Your email verification code")
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

    match (&config.smtp_user, &config.smtp_pass) {
        (Some(user), Some(pass)) => {
            transport_builder =
                transport_builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }
        (None, None) => {}
        _ => return Err("SMTP_USER and SMTP_PASS must either both be set or both be empty".into()),
    }

    let transport = transport_builder.build();

    info!(
        "[EMAIL DISPATCHER] Contacting SMTP host {}:{}...",
        smtp_host, config.smtp_port
    );

    match transport.send(email_msg).await {
        Ok(resp) => {
            info!(
                "[EMAIL DISPATCHER] Successfully delivered OTP email via SMTP (Response: {:?})",
                resp
            );
            Ok(true)
        }
        Err(e) => {
            error!("[EMAIL DISPATCHER] SMTP transmission failed: {}", e);
            Err(e.to_string())
        }
    }
}
