use super::tool_loop::wait_for_cancel;
use super::*;

pub(super) async fn safe_get_with_client(
    client: &Client,
    url: Url,
) -> Result<reqwest::Response, WebSearchError> {
    let mut current = url;
    for redirect_count in 0..=MAX_REDIRECTS {
        let addresses = validate_public_url(&current).await?;
        let response = send_with_pinned_addresses(client, &current, &addresses).await?;
        if !response.status().is_redirection() {
            return Ok(response);
        }
        if redirect_count == MAX_REDIRECTS {
            return Err(WebSearchError::TooManyRedirects);
        }
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(WebSearchError::InvalidUrl)?;
        current = current
            .join(location)
            .map_err(|_| WebSearchError::InvalidUrl)?;
    }
    Err(WebSearchError::TooManyRedirects)
}

pub(super) async fn send_with_pinned_addresses(
    _client: &Client,
    url: &Url,
    addresses: &[SocketAddr],
) -> Result<reqwest::Response, WebSearchError> {
    let host = url.host_str().unwrap_or("").to_string();
    let pinned = PinnedDnsResolver {
        host: host.clone(),
        addresses: addresses.to_vec(),
    };
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("locoryn/", env!("CARGO_PKG_VERSION")))
        .dns_resolver(pinned)
        .build()
        .map_err(|error| WebSearchError::ProviderUnavailable(error.to_string()))?;
    client
        .get(url.clone())
        .send()
        .await
        .map_err(map_reqwest_error)
}

struct PinnedDnsResolver {
    host: String,
    addresses: Vec<SocketAddr>,
}

impl reqwest::dns::Resolve for PinnedDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        if name.as_str() == self.host {
            let addrs: reqwest::dns::Addrs = Box::new(self.addresses.clone().into_iter());
            Box::pin(std::future::ready(Ok(addrs)))
        } else {
            let addrs: reqwest::dns::Addrs = Box::new(std::iter::empty());
            Box::pin(std::future::ready(Ok(addrs)))
        }
    }
}

pub(super) async fn fetch_page_with_client(
    client: &Client,
    url: &str,
) -> Result<WebPageContent, WebSearchError> {
    let parsed = Url::parse(url).map_err(|_| WebSearchError::InvalidUrl)?;
    let response = safe_get_with_client(client, parsed).await?;
    map_status(response.status())?;
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !(content_type.starts_with("text/html")
        || content_type.starts_with("text/plain")
        || content_type.starts_with("application/xhtml+xml"))
    {
        return Err(WebSearchError::UnsupportedContentType);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PAGE_BYTES as u64)
    {
        return Err(WebSearchError::ResponseTooLarge);
    }
    let final_url = response.url().to_string();
    let mut response = response;
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_PAGE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response.chunk().await.map_err(map_reqwest_error)? {
        if chunk.len() > MAX_PAGE_BYTES.saturating_sub(bytes.len()) {
            return Err(WebSearchError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }

    // Detect charset from Content-Type header, then from <meta> tags in the
    // HTML head. Falls back to UTF-8 with replacement when detection fails.
    let encoding = detect_charset(&content_type, &bytes);
    let (raw, _encoding, _had_errors) = encoding.decode(&bytes);
    let raw = raw.into_owned();

    let title = html_title(&raw);
    let text = if content_type.starts_with("text/plain") {
        raw
    } else {
        html_to_text(&raw)
    };
    Ok(WebPageContent {
        url: final_url,
        title,
        text: text.chars().take(MAX_PAGE_BYTES).collect(),
    })
}

pub(super) fn detect_charset(content_type: &str, bytes: &[u8]) -> &'static encoding_rs::Encoding {
    // 1. Try Content-Type header charset parameter
    if let Some(charset) = content_type.split(';').skip(1).find_map(|part| {
        let (key, value) = part.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches('"').trim_matches('\''))
    }) && let Some(encoding) = encoding_rs::Encoding::for_label(charset.as_bytes())
    {
        return encoding;
    }

    // 2. Look for <meta charset> or <meta http-equiv> in the first 4 KiB
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let lowercase = head.to_ascii_lowercase();

    // <meta charset="...">
    if let Some(charset_pos) = lowercase.find("charset") {
        let after = &lowercase[charset_pos + 7..];
        let after = after.trim_start_matches(|ch: char| ch == '=' || ch.is_whitespace());
        let charset = after
            .trim_start_matches('"')
            .trim_start_matches('\'')
            .split(|ch: char| {
                ch == '"' || ch == '\'' || ch == '>' || ch == '/' || ch.is_whitespace()
            })
            .next()
            .unwrap_or("");
        if !charset.is_empty()
            && let Some(encoding) = encoding_rs::Encoding::for_label(charset.as_bytes())
        {
            return encoding;
        }
    }

    // 3. Default to UTF-8 with replacement
    encoding_rs::UTF_8
}

pub(super) fn map_status(status: StatusCode) -> Result<(), WebSearchError> {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(WebSearchError::Unauthorized),
        StatusCode::TOO_MANY_REQUESTS => Err(WebSearchError::RateLimited),
        status if status.is_success() => Ok(()),
        status => Err(WebSearchError::ProviderUnavailable(format!(
            "HTTP {status}"
        ))),
    }
}

pub(super) fn map_reqwest_error(error: reqwest::Error) -> WebSearchError {
    if error.is_timeout() {
        WebSearchError::Timeout
    } else {
        WebSearchError::ProviderUnavailable(error.to_string())
    }
}

pub(super) fn is_transient_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::BAD_GATEWAY
        || status == StatusCode::SERVICE_UNAVAILABLE
        || status == StatusCode::GATEWAY_TIMEOUT
}

pub(super) fn retry_after_delay(response: &reqwest::Response, attempt: usize) -> Duration {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(1_u64 << attempt.min(3)))
        .clamp(Duration::from_millis(500), Duration::from_secs(15))
}

/// Ollama Cloud and proxied Ollama endpoints can briefly answer with a rate
/// limit or gateway error while a model is loading. Retry only transient
/// statuses, respect Retry-After when supplied, and keep every wait cancellable
/// so Stop remains immediate.
pub(crate) async fn send_ollama_request_with_retry(
    client: &Client,
    url: &str,
    body: &serde_json::Value,
    cancel: &AtomicBool,
) -> Result<Option<reqwest::Response>, reqwest::Error> {
    let mut attempt = 0;
    loop {
        let request = client.post(url).json(body).send();
        let response = tokio::select! {
            response = request => response,
            () = wait_for_cancel(cancel) => return Ok(None),
        };
        let response = match response {
            Ok(response) => response,
            Err(error)
                if attempt < MAX_RATE_LIMIT_RETRIES
                    && (error.is_timeout() || error.is_connect()) =>
            {
                let delay = Duration::from_secs(1_u64 << attempt.min(3));
                attempt += 1;
                tokio::select! {
                    () = tokio::time::sleep(delay) => {}
                    () = wait_for_cancel(cancel) => return Ok(None),
                }
                continue;
            }
            Err(error) => return Err(error),
        };
        if !is_transient_status(response.status()) || attempt >= MAX_RATE_LIMIT_RETRIES {
            return Ok(Some(response));
        }

        let delay = retry_after_delay(&response, attempt);
        attempt += 1;
        tokio::select! {
            () = tokio::time::sleep(delay) => {}
            () = wait_for_cancel(cancel) => return Ok(None),
        }
    }
}

pub async fn validate_public_url(url: &Url) -> Result<Vec<SocketAddr>, WebSearchError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(WebSearchError::UnsupportedScheme);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(WebSearchError::InvalidUrl);
    }
    let host = match url.host().ok_or(WebSearchError::InvalidUrl)? {
        Host::Ipv4(ip) => {
            validate_public_ip(IpAddr::V4(ip))?;
            let port = url
                .port_or_known_default()
                .ok_or(WebSearchError::InvalidUrl)?;
            return Ok(vec![SocketAddr::new(IpAddr::V4(ip), port)]);
        }
        Host::Ipv6(ip) => {
            validate_public_ip(IpAddr::V6(ip))?;
            let port = url
                .port_or_known_default()
                .ok_or(WebSearchError::InvalidUrl)?;
            return Ok(vec![SocketAddr::new(IpAddr::V6(ip), port)]);
        }
        Host::Domain(host) => host,
    };
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return Err(WebSearchError::UnsafeAddress);
    }
    let port = url
        .port_or_known_default()
        .ok_or(WebSearchError::InvalidUrl)?;
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| WebSearchError::ProviderUnavailable(error.to_string()))?
        .collect::<Vec<SocketAddr>>();
    if addresses.is_empty() {
        return Err(WebSearchError::InvalidUrl);
    }
    for address in &addresses {
        validate_public_ip(address.ip())?;
    }
    Ok(addresses)
}

pub(super) fn validate_public_ip(ip: IpAddr) -> Result<(), WebSearchError> {
    let unsafe_address = match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 224
                || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| validate_public_ip(IpAddr::V4(mapped)).is_err())
        }
    };
    if unsafe_address {
        Err(WebSearchError::UnsafeAddress)
    } else {
        Ok(())
    }
}

pub(super) fn html_title(html: &str) -> Option<String> {
    let lowercase = html.to_ascii_lowercase();
    let start = lowercase.find("<title")?;
    let open_end = lowercase[start..].find('>')? + start + 1;
    let end = lowercase[open_end..].find("</title>")? + open_end;
    let title = decode_html_entities(html[open_end..end].trim());
    (!title.is_empty()).then_some(title)
}

pub(super) fn html_to_text(html: &str) -> String {
    let lowercase = html.to_ascii_lowercase();

    let mut sanitized = String::with_capacity(html.len());
    let mut cursor = 0;
    let block_tags: &[(&str, &str)] = &[
        ("<script", "</script>"),
        ("<style", "</style>"),
        ("<nav", "</nav>"),
        ("<header", "</header>"),
        ("<footer", "</footer>"),
        ("<aside", "</aside>"),
        ("<noscript", "</noscript>"),
    ];

    loop {
        let remaining_lower = &lowercase[cursor..];
        let mut earliest: Option<(usize, usize)> = None;

        // Strip known non-content block elements
        for (open, close) in block_tags {
            if let Some(start) = remaining_lower.find(open)
                && let Some(end) = lowercase[cursor + start..]
                    .find(close)
                    .map(|pos| cursor + start + pos + close.len())
                && earliest.is_none_or(|(earliest_start, _)| start < earliest_start)
            {
                earliest = Some((start, end));
            }
        }

        // Strip elements with hidden / aria-hidden / non-content roles
        let non_content_attrs: &[&str] = &[
            "hidden",
            "aria-hidden=\"true\"",
            "aria-hidden='true'",
            "role=\"banner\"",
            "role='banner'",
            "role=\"navigation\"",
            "role='navigation'",
            "role=\"complementary\"",
            "role='complementary'",
            "role=\"contentinfo\"",
            "role='contentinfo'",
        ];
        for attr in non_content_attrs {
            if let Some(attr_pos) = remaining_lower.find(attr) {
                let tag_start = remaining_lower[..attr_pos].rfind('<').unwrap_or(attr_pos);
                let tag_name_end = remaining_lower[tag_start..]
                    .find(|ch: char| ch == '>' || ch.is_whitespace())
                    .map(|pos| tag_start + pos)
                    .unwrap_or(tag_start + 1);
                let tag_name = &lowercase[tag_start + 1..tag_name_end];
                let close_tag = format!("</{tag_name}>");
                if let Some(end) = lowercase[cursor + tag_name_end..]
                    .find(&close_tag)
                    .map(|pos| cursor + tag_name_end + pos + close_tag.len())
                {
                    let start = cursor + tag_start;
                    if earliest.is_none() || start - cursor < earliest.unwrap().0 {
                        earliest = Some((start - cursor, end));
                    }
                }
            }
        }

        // Strip elements with display:none / visibility:hidden in inline style
        if let Some(style_pos) = remaining_lower
            .find("display:none")
            .or_else(|| remaining_lower.find("display: none"))
            .or_else(|| remaining_lower.find("visibility:hidden"))
            .or_else(|| remaining_lower.find("visibility: hidden"))
        {
            let tag_start = remaining_lower[..style_pos].rfind('<').unwrap_or(style_pos);
            let tag_name_end = remaining_lower[tag_start..]
                .find(|ch: char| ch == '>' || ch.is_whitespace())
                .map(|pos| tag_start + pos)
                .unwrap_or(tag_start + 1);
            let tag_name = &lowercase[tag_start + 1..tag_name_end];
            let close_tag = format!("</{tag_name}>");
            if let Some(end) = lowercase[cursor + tag_name_end..]
                .find(&close_tag)
                .map(|pos| cursor + tag_name_end + pos + close_tag.len())
            {
                let start = cursor + tag_start;
                if earliest.is_none() || start - cursor < earliest.unwrap().0 {
                    earliest = Some((start - cursor, end));
                }
            }
        }

        match earliest {
            Some((start, end)) => {
                sanitized.push_str(&html[cursor..cursor + start]);
                cursor = end;
            }
            None => break,
        }
    }
    sanitized.push_str(&html[cursor..]);

    let mut text = String::with_capacity(sanitized.len());
    let mut in_tag = false;
    for character in sanitized.chars() {
        match character {
            '<' => {
                in_tag = true;
                text.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    decode_html_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
