use super::*;

#[derive(Clone)]
pub struct BraveSearchProvider {
    client: Client,
    api_key: String,
    search_endpoint: Url,
}

impl BraveSearchProvider {
    pub fn new(settings: &WebSearchSettings) -> Result<Self, WebSearchError> {
        Self::with_endpoint(settings, "https://api.search.brave.com/res/v1/web/search")
    }

    fn with_endpoint(settings: &WebSearchSettings, endpoint: &str) -> Result<Self, WebSearchError> {
        let api_key = provider_api_key(settings, WebSearchProviderKind::Brave)?;
        let client = provider_http_client(settings)?;
        let search_endpoint = Url::parse(endpoint).map_err(|_| WebSearchError::InvalidUrl)?;
        Ok(Self {
            client,
            api_key,
            search_endpoint,
        })
    }
}

#[derive(Deserialize)]
pub(super) struct BraveResponse {
    web: Option<BraveWebResults>,
}

#[derive(Deserialize)]
struct BraveWebResults {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
}

#[async_trait]
impl WebSearchProvider for BraveSearchProvider {
    async fn search(
        &self,
        query: &str,
        limit: usize,
        freshness: WebSearchFreshness,
    ) -> Result<Vec<WebSearchResult>, WebSearchError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(WebSearchError::EmptyResults);
        }
        let mut endpoint = self.search_endpoint.clone();
        endpoint
            .query_pairs_mut()
            .append_pair("q", query)
            .append_pair("count", &limit.clamp(1, MAX_RESULT_LIMIT).to_string());
        if let Some(freshness) = freshness.brave_value() {
            endpoint
                .query_pairs_mut()
                .append_pair("freshness", freshness);
        }
        let response = send_search_request_with_retry(
            self.client
                .get(endpoint)
                .header("X-Subscription-Token", &self.api_key)
                .header(header::ACCEPT, "application/json"),
            &BRAVE_SEARCH_PACER,
        )
        .await?;
        map_status(response.status())?;
        let body: BraveResponse = response
            .json()
            .await
            .map_err(|error| WebSearchError::ProviderUnavailable(error.to_string()))?;
        parse_brave_results(body, limit)
    }

    async fn fetch_page(&self, url: &str) -> Result<WebPageContent, WebSearchError> {
        fetch_page_with_client(&self.client, url).await
    }
}

pub(super) fn parse_brave_results(
    body: BraveResponse,
    limit: usize,
) -> Result<Vec<WebSearchResult>, WebSearchError> {
    let results = body
        .web
        .map(|web| web.results)
        .unwrap_or_default()
        .into_iter()
        .take(limit.clamp(1, MAX_RESULT_LIMIT))
        .filter(|result| {
            Url::parse(&result.url)
                .ok()
                .is_some_and(|url| matches!(url.scheme(), "http" | "https"))
        })
        .map(|result| WebSearchResult {
            title: result.title,
            url: result.url,
            snippet: result.description,
        })
        .collect::<Vec<_>>();
    if results.is_empty() {
        Err(WebSearchError::EmptyResults)
    } else {
        Ok(results)
    }
}

#[derive(Clone)]
pub struct TavilySearchProvider {
    client: Client,
    api_key: String,
    search_endpoint: Url,
}

impl TavilySearchProvider {
    pub fn new(settings: &WebSearchSettings) -> Result<Self, WebSearchError> {
        Self::with_endpoint(settings, "https://api.tavily.com/search")
    }

    fn with_endpoint(settings: &WebSearchSettings, endpoint: &str) -> Result<Self, WebSearchError> {
        Ok(Self {
            client: provider_http_client(settings)?,
            api_key: provider_api_key(settings, WebSearchProviderKind::Tavily)?,
            search_endpoint: Url::parse(endpoint).map_err(|_| WebSearchError::InvalidUrl)?,
        })
    }
}

#[derive(Deserialize)]
pub(super) struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
}

pub(super) fn tavily_request_body(
    query: &str,
    limit: usize,
    freshness: WebSearchFreshness,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "query": query,
        "search_depth": "basic",
        "max_results": limit.clamp(1, MAX_RESULT_LIMIT),
        "include_answer": false,
        "include_raw_content": false,
        "include_images": false,
    });
    if let Some(time_range) = freshness.tavily_value() {
        body["time_range"] = serde_json::Value::String(time_range.to_string());
    }
    body
}

#[async_trait]
impl WebSearchProvider for TavilySearchProvider {
    async fn search(
        &self,
        query: &str,
        limit: usize,
        freshness: WebSearchFreshness,
    ) -> Result<Vec<WebSearchResult>, WebSearchError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(WebSearchError::EmptyResults);
        }
        let response = send_search_request_with_retry(
            self.client
                .post(self.search_endpoint.clone())
                .bearer_auth(&self.api_key)
                .header(header::ACCEPT, "application/json")
                .json(&tavily_request_body(query, limit, freshness)),
            &TAVILY_SEARCH_PACER,
        )
        .await?;
        map_status(response.status())?;
        let body: TavilyResponse = response
            .json()
            .await
            .map_err(|error| WebSearchError::ProviderUnavailable(error.to_string()))?;
        parse_tavily_results(body, limit)
    }

    async fn fetch_page(&self, url: &str) -> Result<WebPageContent, WebSearchError> {
        fetch_page_with_client(&self.client, url).await
    }
}

pub(super) fn parse_tavily_results(
    body: TavilyResponse,
    limit: usize,
) -> Result<Vec<WebSearchResult>, WebSearchError> {
    collect_valid_results(
        body.results.into_iter().map(|result| WebSearchResult {
            title: result.title,
            url: result.url,
            snippet: result.content,
        }),
        limit,
    )
}

#[derive(Clone)]
pub struct ExaSearchProvider {
    client: Client,
    api_key: String,
    search_endpoint: Url,
}

impl ExaSearchProvider {
    pub fn new(settings: &WebSearchSettings) -> Result<Self, WebSearchError> {
        Self::with_endpoint(settings, "https://api.exa.ai/search")
    }

    fn with_endpoint(settings: &WebSearchSettings, endpoint: &str) -> Result<Self, WebSearchError> {
        Ok(Self {
            client: provider_http_client(settings)?,
            api_key: provider_api_key(settings, WebSearchProviderKind::Exa)?,
            search_endpoint: Url::parse(endpoint).map_err(|_| WebSearchError::InvalidUrl)?,
        })
    }
}

#[derive(Deserialize)]
pub(super) struct ExaResponse {
    #[serde(default)]
    results: Vec<ExaResult>,
}

#[derive(Deserialize)]
struct ExaResult {
    title: Option<String>,
    url: String,
    #[serde(default)]
    highlights: Vec<String>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    summary: String,
}

pub(super) fn exa_request_body(
    query: &str,
    limit: usize,
    freshness: WebSearchFreshness,
    now: chrono::DateTime<chrono::Utc>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "query": query,
        "numResults": limit.clamp(1, MAX_RESULT_LIMIT),
        "type": "auto",
        "contents": {
            "highlights": true
        }
    });
    if let Some(days) = freshness.exa_lookback_days() {
        body["startPublishedDate"] = serde_json::Value::String(
            (now - chrono::Duration::days(days))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        );
    }
    body
}

#[async_trait]
impl WebSearchProvider for ExaSearchProvider {
    async fn search(
        &self,
        query: &str,
        limit: usize,
        freshness: WebSearchFreshness,
    ) -> Result<Vec<WebSearchResult>, WebSearchError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(WebSearchError::EmptyResults);
        }
        let response = send_search_request_with_retry(
            self.client
                .post(self.search_endpoint.clone())
                .header("x-api-key", &self.api_key)
                .header(header::ACCEPT, "application/json")
                .json(&exa_request_body(
                    query,
                    limit,
                    freshness,
                    chrono::Utc::now(),
                )),
            &EXA_SEARCH_PACER,
        )
        .await?;
        map_status(response.status())?;
        let body: ExaResponse = response
            .json()
            .await
            .map_err(|error| WebSearchError::ProviderUnavailable(error.to_string()))?;
        parse_exa_results(body, limit)
    }

    async fn fetch_page(&self, url: &str) -> Result<WebPageContent, WebSearchError> {
        fetch_page_with_client(&self.client, url).await
    }
}

pub(super) fn parse_exa_results(
    body: ExaResponse,
    limit: usize,
) -> Result<Vec<WebSearchResult>, WebSearchError> {
    collect_valid_results(
        body.results.into_iter().map(|result| {
            let snippet = if result.highlights.is_empty() {
                if result.text.is_empty() {
                    result.summary
                } else {
                    result.text
                }
            } else {
                result.highlights.join(" [...] ")
            };
            WebSearchResult {
                title: result.title.unwrap_or_else(|| "Untitled result".into()),
                url: result.url,
                snippet,
            }
        }),
        limit,
    )
}

pub(super) fn collect_valid_results(
    results: impl IntoIterator<Item = WebSearchResult>,
    limit: usize,
) -> Result<Vec<WebSearchResult>, WebSearchError> {
    let results = results
        .into_iter()
        .filter(|result| {
            Url::parse(&result.url)
                .ok()
                .is_some_and(|url| matches!(url.scheme(), "http" | "https"))
        })
        .take(limit.clamp(1, MAX_RESULT_LIMIT))
        .collect::<Vec<_>>();
    if results.is_empty() {
        Err(WebSearchError::EmptyResults)
    } else {
        Ok(results)
    }
}
