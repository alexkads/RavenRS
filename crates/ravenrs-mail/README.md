# ravenrs-mail

Sending email from a [RavenRS](https://github.com/alexkads/RavenRS) service: a message with its attachments, through an SMTP server.

## Role

A service that tells people something by email needs the same few things: a message whose subject cannot turn into another header, attachments, and a connection to a mail server over TLS. This crate is only that. It knows nothing of where the message comes from. `ravenrs-report-server` sends scheduled reports with it, and the plugin `email` of Raven CI tells when a build fails.

## How it works

- `Mail` is a message: recipients, subject, text and `MailAttachment`s (name, media type and bytes).
- `Mailer` sends one. `SmtpMailer` does it through an SMTP server with `lettre`, over STARTTLS (port 587), TLS (port 465), or nothing at all (`Security::None`), which you choose on purpose for a relay on the same machine or a test server.
- `SmtpSettings` takes credentials that are already read. Where they come from (environment variables, a vault) is up to the caller, and a debug print never shows the password.
- Sending blocks, so call it off the async threads.
- A line break in the subject cannot add a header such as `Bcc`. A message with no recipient, or with an address that does not parse, is an error before anything is sent.

## Use

```rust,no_run
use ravenrs_mail::{Mail, MailAttachment, Mailer, Security, SmtpMailer, SmtpSettings};

let settings = SmtpSettings::new("smtp.example.com", "CI <ci@example.com>")
    .security(Security::Starttls)
    .credentials("ci@example.com", "secret");
let mailer = SmtpMailer::new(&settings)?;
let mail = Mail::new(["ana@example.com"], "Relatório de vendas", "Segue o relatório.")
    .attach(MailAttachment::new("vendas.csv", "text/csv; charset=utf-8", b"mes,total\n".to_vec()));
mailer.send(&mail)?;
# Ok::<(), String>(())
```

In tests, implement `Mailer` on a type of your own that keeps the messages instead of sending them.

## License

MIT OR Apache-2.0, as the rest of RavenRS.
