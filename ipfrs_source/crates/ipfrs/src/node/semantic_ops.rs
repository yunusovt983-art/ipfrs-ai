//! Semantic search and vector index operations for Node

use ipfrs_core::{Cid, Result};
use ipfrs_semantic::{QueryFilter, SearchResult};
use std::path::Path;

use super::{Node, SemanticStats};

impl Node {
    /// Get semantic router statistics
    ///
    /// Returns comprehensive statistics about the semantic index including
    /// vector count, dimension, distance metric, and cache performance.
    ///
    /// # Returns
    /// Statistics about the semantic router
    ///
    /// # Errors
    /// Returns error if semantic routing is not enabled
    ///
    /// # Example
    /// ```rust,no_run
    /// use ipfrs::{Node, NodeConfig};
    ///
    /// # async fn example() -> ipfrs::Result<()> {
    /// let mut node = Node::new(NodeConfig::default())?;
    /// node.start().await?;
    ///
    /// let stats = node.semantic_stats()?;
    /// println!("Indexed vectors: {}", stats.num_vectors);
    /// println!("Vector dimension: {}", stats.dimension);
    /// println!("Cache size: {}/{}", stats.cache_size, stats.cache_capacity);
    /// # Ok(())
    /// # }
    /// ```
    pub fn semantic_stats(&self) -> Result<SemanticStats> {
        let semantic = self.semantic()?;

        let router_stats = semantic.stats();
        let cache_stats = semantic.cache_stats();

        Ok(SemanticStats {
            num_vectors: router_stats.num_vectors,
            dimension: router_stats.dimension,
            metric: router_stats.metric,
            cache_size: cache_stats.size,
            cache_capacity: cache_stats.capacity,
        })
    }

    /// Index content with its semantic embedding
    ///
    /// Adds content to the semantic index for similarity search. The embedding
    /// should be a vector representation of the content (e.g., from a sentence
    /// transformer model).
    ///
    /// # Arguments
    /// * `cid` - Content identifier to index
    /// * `embedding` - Vector embedding of the content
    ///
    /// # Example
    /// ```rust,no_run
    /// use ipfrs::{Node, NodeConfig};
    ///
    /// # async fn example() -> ipfrs::Result<()> {
    /// let mut node = Node::new(NodeConfig::default())?;
    /// node.start().await?;
    ///
    /// # let cid = ipfrs_core::Cid::default();
    /// // Index content with 768-dimensional embedding (e.g., from BERT)
    /// let embedding = vec![0.5; 768];
    /// node.index_content(&cid, &embedding).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn index_content(&self, cid: &Cid, embedding: &[f32]) -> Result<()> {
        let semantic = self.semantic()?;
        semantic.add(cid, embedding)
    }

    /// Search for similar content by semantic similarity
    ///
    /// Performs k-nearest neighbor search over indexed content using vector
    /// similarity. Returns the top k most similar items.
    ///
    /// # Arguments
    /// * `query_embedding` - Query vector to search for
    /// * `k` - Number of results to return
    ///
    /// # Returns
    /// Vector of search results ordered by similarity (highest first)
    ///
    /// # Example
    /// ```rust,no_run
    /// use ipfrs::{Node, NodeConfig};
    ///
    /// # async fn example() -> ipfrs::Result<()> {
    /// let mut node = Node::new(NodeConfig::default())?;
    /// node.start().await?;
    ///
    /// // Search for top 10 similar documents
    /// let query_embedding = vec![0.3; 768];
    /// let results = node.search_similar(&query_embedding, 10).await?;
    ///
    /// for result in results {
    ///     println!("CID: {}, Score: {}", result.cid, result.score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search_similar(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<SearchResult>> {
        let semantic = self.semantic()?;
        semantic.query(query_embedding, k).await
    }

    /// Search with advanced filtering options
    ///
    /// Performs semantic search with additional filters like minimum score
    /// threshold, CID prefix matching, and result limits.
    ///
    /// # Arguments
    /// * `query_embedding` - Query vector to search for
    /// * `k` - Number of results to return
    /// * `filter` - Query filter options
    ///
    /// # Returns
    /// Vector of filtered search results ordered by similarity
    ///
    /// # Example
    /// ```rust,no_run
    /// use ipfrs::{Node, NodeConfig, QueryFilter};
    ///
    /// # async fn example() -> ipfrs::Result<()> {
    /// let mut node = Node::new(NodeConfig::default())?;
    /// node.start().await?;
    ///
    /// // Search with filters
    /// let query_embedding = vec![0.3; 768];
    /// let filter = QueryFilter {
    ///     min_score: Some(0.8),  // Only results with score >= 0.8
    ///     max_score: None,        // No max score filter
    ///     max_results: Some(5),   // Limit to 5 results
    ///     cid_prefix: None,       // No CID filtering
    /// };
    ///
    /// let results = node.search_hybrid(&query_embedding, 20, filter).await?;
    ///
    /// for result in results {
    ///     println!("High-confidence match: {} ({})", result.cid, result.score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search_hybrid(
        &self,
        query_embedding: &[f32],
        k: usize,
        filter: QueryFilter,
    ) -> Result<Vec<SearchResult>> {
        let semantic = self.semantic()?;
        semantic.query_with_filter(query_embedding, k, filter).await
    }

    /// Save the semantic index to disk
    ///
    /// Persists the entire HNSW index including all vectors and CID mappings
    /// to a file for later loading.
    ///
    /// # Arguments
    /// * `path` - Path to save the index file
    ///
    /// # Example
    /// ```rust,no_run
    /// use ipfrs::{Node, NodeConfig};
    ///
    /// # async fn example() -> ipfrs::Result<()> {
    /// let mut node = Node::new(NodeConfig::default())?;
    /// node.start().await?;
    ///
    /// // Save the semantic index
    /// node.save_semantic_index("semantic.index").await?;
    /// println!("Semantic index saved");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn save_semantic_index(&self, path: impl AsRef<Path>) -> Result<()> {
        let semantic = self.semantic()?;
        semantic.save_index(path).await
    }

    /// Load a semantic index from disk
    ///
    /// Loads a previously saved HNSW index from disk, replacing the current index.
    ///
    /// # Arguments
    /// * `path` - Path to the saved index file
    ///
    /// # Example
    /// ```rust,no_run
    /// use ipfrs::{Node, NodeConfig};
    ///
    /// # async fn example() -> ipfrs::Result<()> {
    /// let mut node = Node::new(NodeConfig::default())?;
    /// node.start().await?;
    ///
    /// // Load the semantic index
    /// node.load_semantic_index("semantic.index").await?;
    /// println!("Semantic index loaded");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn load_semantic_index(&self, path: impl AsRef<Path>) -> Result<()> {
        let semantic = self.semantic()?;
        semantic.load_index(path).await
    }

    // ── Distributed semantic search (RoadMap Phase 1.3) ─────────────────────

    /// Wire inbound semantic-search requests (`/ipfrs/semsearch/1.0.0`) to the
    /// local index, so peers can query this node. Forces semantic init.
    pub fn enable_distributed_semantic(&self) -> Result<()> {
        let router = std::sync::Arc::clone(self.semantic()?);
        self.network()?.set_semsearch_provider(std::sync::Arc::new(
            move |embedding: Vec<f32>, k: u32| {
                let router = router.clone();
                Box::pin(async move {
                    router
                        .query(&embedding, k as usize)
                        .await
                        .map(|res| {
                            res.into_iter()
                                .map(|r| (r.cid.to_string(), r.score))
                                .collect::<Vec<(String, f32)>>()
                        })
                        .unwrap_or_default()
                })
                    as std::pin::Pin<
                        Box<dyn std::future::Future<Output = Vec<(String, f32)>> + Send>,
                    >
            },
        ));
        Ok(())
    }

    /// Distributed k-NN search: query the local index plus all connected peers
    /// over the wire, fuse the per-source rankings with **Reciprocal Rank
    /// Fusion**, and return the top-`k` as `(cid, score)` (RoadMap Phase 1.3).
    ///
    /// Each source (the local index and every peer) is treated as an
    /// independent ranked list. RRF combines them by rank rather than by raw
    /// score, so heterogeneous score scales across peers no longer need
    /// normalization, and a CID that ranks well on several peers outranks one
    /// that merely has a single high score. This reuses the retrieval
    /// subdomain's existing [`ResultAggregator`] instead of a naive best-score
    /// merge. The returned `score` is the fused RRF score (not a cosine
    /// similarity).
    pub async fn semantic_search_distributed(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>> {
        use ipfrs_network::CallResult;
        use ipfrs_semantic::{
            AggAggregationStrategy, AggSearchResult, AggregatorConfig, ResultAggregator,
        };
        use std::collections::HashMap;
        use std::time::{Instant, SystemTime, UNIX_EPOCH};

        // Epoch-millis clock the circuit breaker reasons about (timeouts, cooldowns).
        let now_ms = || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        };

        let mut aggregator = ResultAggregator::new(AggregatorConfig {
            strategy: AggAggregationStrategy::RankFusion,
            max_results: k,
            ..AggregatorConfig::default()
        });

        let into_agg = |source: &str, hits: Vec<(String, f32)>| -> Vec<AggSearchResult> {
            hits.into_iter()
                .map(|(cid, score)| AggSearchResult {
                    doc_id: cid,
                    score: score as f64,
                    source: source.to_string(),
                    metadata: HashMap::new(),
                })
                .collect()
        };

        // Local results (best-effort; the semantic index may be empty).
        if let Ok(local) = self.search_similar(query, k).await {
            let hits = local
                .into_iter()
                .map(|r| (r.cid.to_string(), r.score))
                .collect();
            aggregator.add_results("local", into_agg("local", hits));
        }

        // Peer results over /ipfrs/semsearch/1.0.0 — each peer is its own source,
        // guarded by a per-peer circuit breaker so a flaky peer is skipped while
        // its breaker is Open instead of being re-queried on every search.
        let network = self.network()?;
        for peer in network.connected_peers() {
            let source = peer.to_string();

            // Skip peers whose breaker is Open (cooldown not yet elapsed).
            if !self
                .semsearch_breaker
                .lock()
                .can_call(&source, now_ms())
            {
                continue;
            }

            let started = Instant::now();
            let outcome = network
                .query_peer_semantic(&peer, query.to_vec(), k as u32)
                .await;
            let elapsed_ms = started.elapsed().as_millis() as u64;

            let call_result = match &outcome {
                Ok(_) => CallResult::Success {
                    duration_ms: elapsed_ms,
                },
                Err(e) => {
                    let reason = e.to_string();
                    if reason.contains("timed out") {
                        CallResult::Timeout {
                            duration_ms: elapsed_ms,
                        }
                    } else {
                        CallResult::Failure {
                            duration_ms: elapsed_ms,
                            reason,
                        }
                    }
                }
            };
            self.semsearch_breaker
                .lock()
                .record_result(&source, call_result, now_ms());

            if let Ok(hits) = outcome {
                let results = into_agg(&source, hits);
                aggregator.add_results(&source, results);
            }
        }

        let merged = aggregator
            .aggregate()
            .into_iter()
            .map(|r| (r.doc_id, r.final_score as f32))
            .collect();
        Ok(merged)
    }
}
