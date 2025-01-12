use crate::{
    code::{self, IRawGalleryExtensionsResult, IRawGalleryQueryResult, TargetPlatform},
    config::Extension,
    error::Error,
};

use super::Query;
use async_stream::try_stream;
use futures::stream::Stream;
use tracing::*;

#[derive(Debug, Clone)]
pub struct HttpClient {
    pub client: reqwest::Client,
}

impl HttpClient {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder().gzip(true).build()?;
        Ok(Self { client })
    }

    pub fn get_extension_response(
        &self,
        extensions: Vec<Extension>,
    ) -> impl Stream<Item = anyhow::Result<code::IRawGalleryExtensionsResult>> + '_ {
        let mut page_number: u64 = 1;

        try_stream! {
            loop {
                let query = Query::new(&extensions, page_number);
                let body = serde_json::to_string(&query)?;
                trace!("send request: {body}");
                let response = self
                    .client
                    .post("https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery")
                    .header(
                        "Accept",
                        "Application/json; charset=utf-8; api-version=7.2-preview.1",
                    )
                    .header("Content-Type", "application/json")
                    .body(body.clone())
                    .send()
                    .await?
                    .json::<IRawGalleryQueryResult>()
                    .await?;

                if response.results.is_empty() {
                    break;
                }

                for item in response.results {
                    yield item
                }

                page_number += 1;
            }
        }
    }

    async fn inner_get_extension_target_platform(
        &self,
        publisher_name: String,
        extension_name: String,
    ) -> anyhow::Result<IRawGalleryExtensionsResult> {
        let query = Query::create_search(publisher_name, extension_name);
        let body = serde_json::to_string(&query)?;
        let txt = self
            .client
            .post("https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery")
            .header(
                "Accept",
                "Application/json; charset=utf-8; api-version=7.2-preview.1",
            )
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            .json::<code::IRawGalleryQueryResult>()
            .await?;
        txt.results.into_iter().next().ok_or(Error::Unknown.into())
    }

    pub async fn get_extension_target_platform(
        &self,
        publisher_name: String,
        extension_name: String,
    ) -> Vec<TargetPlatform> {
        trace!("get target_platform of {publisher_name}.{extension_name}");
        match self
            .inner_get_extension_target_platform(publisher_name, extension_name)
            .await
        {
            Ok(res) => {
                let i: Vec<_> = res
                    .get_target_platform()
                    .into_iter()
                    .filter(|item| !matches!(item, TargetPlatform::Unknown))
                    .collect();

                let j: Vec<_> = i
                    .iter()
                    .filter(|item| {
                        !matches!(*item, TargetPlatform::Web | TargetPlatform::Universal)
                    })
                    .copied()
                    .collect();

                if !j.is_empty() {
                    j
                } else {
                    i
                }
            }
            Err(err) => {
                error!("Error happend when get target_platform: {err}");
                vec![]
            }
        }
    }
}
