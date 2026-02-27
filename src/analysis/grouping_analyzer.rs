use crate::schema::Column;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupingAnalysis {
    pub table_name: String,
    pub grouping_dimensions: Vec<GroupingDimension>,
    pub hierarchies: Vec<GroupingHierarchy>,
    pub suggested_analyses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupingDimension {
    pub column_name: String,
    pub dimension_type: DimensionType,
    pub group_count: usize,
    pub records_per_group: GroupStats,
    pub example_groups: Vec<GroupExample>,
    pub insights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupExample {
    pub group_value: String,
    pub record_count: usize,
    pub sample_records: Vec<usize>, // row indices
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupStats {
    pub min: usize,
    pub max: usize,
    pub avg: f64,
    pub median: usize,
    pub total_records: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DimensionType {
    Temporal,      // Date, time-based grouping
    Categorical,   // Status, category grouping
    Geographic,    // Location-based
    Hierarchical,  // Parent-child relationships
    Identifier,    // Customer ID, Product ID
    Numeric,       // Can be binned for analysis
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupingHierarchy {
    pub levels: Vec<String>, // Column names from top to bottom
    pub hierarchy_type: HierarchyType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HierarchyType {
    Temporal,    // Year → Month → Day
    Geographic,  // Country → State → City
    Categorical, // Category → Subcategory → Item
    Organizational, // Department → Team → Employee
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationPattern {
    pub group_by: Vec<String>,
    pub aggregate_columns: Vec<String>,
    pub pattern_type: AggregationType,
    pub description: String,
    pub sample_result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AggregationType {
    Sum,
    Count,
    Average,
    MinMax,
    Distinct,
}

pub struct GroupingAnalyzer {
    min_groups: usize,
    max_groups: usize,
    sample_size: usize,
}

impl GroupingAnalyzer {
    pub fn new(sample_size: usize) -> Self {
        Self {
            min_groups: 1,       // Include all groupable columns
            max_groups: 1000,    // Don't analyze if too many unique values
            sample_size,
        }
    }

    /// Extract composite parts from a value (e.g., "T-Hemp - Branders" -> ["T-Hemp", "Branders"])
    fn extract_composite_parts(value: &str) -> Vec<String> {
        let trimmed = value.trim();

        // Common separators for composite values
        let separators = [" - ", " / ", " | ", "\n", ";"];

        // Check each separator
        for sep in &separators {
            if trimmed.contains(sep) {
                return trimmed
                    .split(sep)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }

        // If no separator found, return original value
        vec![trimmed.to_string()]
    }

    pub fn analyze_groupings(
        &self,
        data: &[Vec<String>],
        columns: &[Column],
    ) -> GroupingAnalysis {
        let mut grouping_dimensions = Vec::new();
        let mut hierarchies = Vec::new();
        let mut suggested_analyses = Vec::new();

        // Analyze each column as a potential grouping dimension
        for (col_idx, column) in columns.iter().enumerate() {
            if let Some(dimension) = self.analyze_dimension(data, col_idx, column) {
                grouping_dimensions.push(dimension);
            }
        }

        // Find hierarchical relationships between columns
        hierarchies.extend(self.detect_hierarchies(data, columns, &grouping_dimensions));

        // Generate suggested analyses
        suggested_analyses.extend(self.suggest_analyses(&grouping_dimensions, columns));

        GroupingAnalysis {
            table_name: String::new(),
            grouping_dimensions,
            hierarchies,
            suggested_analyses,
        }
    }

    fn analyze_dimension(
        &self,
        data: &[Vec<String>],
        col_idx: usize,
        column: &Column,
    ) -> Option<GroupingDimension> {
        // Build groups
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();

        for (row_idx, row) in data.iter().enumerate().take(self.sample_size) {
            if let Some(value) = row.get(col_idx) {
                if !value.is_empty() {
                    // Check if value contains composite parts
                    let parts = Self::extract_composite_parts(value);

                    if parts.len() > 1 {
                        // If composite, add each part as a separate group
                        for part in parts {
                            groups
                                .entry(part)
                                .or_insert_with(Vec::new)
                                .push(row_idx);
                        }
                    } else {
                        // Single value, use as-is
                        groups
                            .entry(value.clone())
                            .or_insert_with(Vec::new)
                            .push(row_idx);
                    }
                }
            }
        }

        let group_count = groups.len();

        // Filter out columns that aren't good for grouping
        if group_count < self.min_groups || group_count > self.max_groups {
            return None;
        }

        // Calculate statistics
        let group_sizes: Vec<usize> = groups.values().map(|v| v.len()).collect();
        let total_records: usize = group_sizes.iter().sum();
        let min = *group_sizes.iter().min().unwrap_or(&0);
        let max = *group_sizes.iter().max().unwrap_or(&0);
        let avg = total_records as f64 / group_count as f64;

        let mut sorted_sizes = group_sizes.clone();
        sorted_sizes.sort();
        let median = if sorted_sizes.is_empty() {
            0
        } else {
            sorted_sizes[sorted_sizes.len() / 2]
        };

        let records_per_group = GroupStats {
            min,
            max,
            avg,
            median,
            total_records,
        };

        // Get example groups (top 5 by size)
        let mut group_list: Vec<(String, Vec<usize>)> = groups.into_iter().collect();
        group_list.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        let example_groups: Vec<GroupExample> = group_list
            .into_iter()
            .take(5)
            .map(|(value, indices)| GroupExample {
                group_value: value,
                record_count: indices.len(),
                sample_records: indices.into_iter().take(3).collect(),
            })
            .collect();

        // Determine dimension type
        let dimension_type = self.classify_dimension(&column.name, &example_groups);

        // Generate insights
        let insights = self.generate_insights(
            &column.name,
            &dimension_type,
            group_count,
            &records_per_group,
        );

        Some(GroupingDimension {
            column_name: column.name.clone(),
            dimension_type,
            group_count,
            records_per_group,
            example_groups,
            insights,
        })
    }

    fn classify_dimension(&self, col_name: &str, examples: &[GroupExample]) -> DimensionType {
        let lower = col_name.to_lowercase();

        // Check for temporal indicators
        if lower.contains("date")
            || lower.contains("time")
            || lower.contains("year")
            || lower.contains("month")
            || lower.contains("day")
        {
            return DimensionType::Temporal;
        }

        // Check for geographic indicators
        if lower.contains("country")
            || lower.contains("state")
            || lower.contains("city")
            || lower.contains("region")
            || lower.contains("location")
        {
            return DimensionType::Geographic;
        }

        // Check for identifier patterns
        if lower.contains("id")
            || lower.contains("customer")
            || lower.contains("user")
            || lower.contains("product")
            || lower.contains("order")
        {
            return DimensionType::Identifier;
        }

        // Check for categorical patterns
        if lower.contains("status")
            || lower.contains("type")
            || lower.contains("category")
            || lower.contains("class")
        {
            return DimensionType::Categorical;
        }

        // Check if values suggest hierarchy
        let has_hierarchy = examples.iter().any(|ex| {
            ex.group_value.contains('/') || ex.group_value.contains('>')
        });
        if has_hierarchy {
            return DimensionType::Hierarchical;
        }

        // Default to categorical
        DimensionType::Categorical
    }

    fn generate_insights(
        &self,
        col_name: &str,
        dim_type: &DimensionType,
        group_count: usize,
        stats: &GroupStats,
    ) -> Vec<String> {
        let mut insights = Vec::new();

        // Basic insight
        insights.push(format!(
            "Can group {} records into {} groups by {}",
            stats.total_records, group_count, col_name
        ));

        // Distribution insight
        if stats.max as f64 > stats.avg * 3.0 {
            insights.push(format!(
                "Uneven distribution: largest group has {} records, average is {:.0}",
                stats.max, stats.avg
            ));
        } else {
            insights.push(format!(
                "Even distribution: {:.0} records per group on average",
                stats.avg
            ));
        }

        // Type-specific insights
        match dim_type {
            DimensionType::Temporal => {
                insights.push(format!(
                    "Time-based analysis possible: trends, seasonality, growth"
                ));
            }
            DimensionType::Identifier => {
                insights.push(format!(
                    "Analyze per {}: lifetime value, behavior patterns, segmentation",
                    col_name
                ));
            }
            DimensionType::Categorical => {
                insights.push(format!(
                    "Compare across {}: performance metrics, distributions",
                    col_name
                ));
            }
            DimensionType::Geographic => {
                insights.push(format!("Geographic analysis: regional patterns, heatmaps"));
            }
            _ => {}
        }

        insights
    }

    fn detect_hierarchies(
        &self,
        data: &[Vec<String>],
        columns: &[Column],
        dimensions: &[GroupingDimension],
    ) -> Vec<GroupingHierarchy> {
        let mut hierarchies = Vec::new();

        // Look for temporal hierarchies (Year → Month → Day)
        let temporal_cols: Vec<&GroupingDimension> = dimensions
            .iter()
            .filter(|d| d.dimension_type == DimensionType::Temporal)
            .collect();

        if temporal_cols.len() >= 2 {
            hierarchies.push(GroupingHierarchy {
                levels: temporal_cols.iter().map(|d| d.column_name.clone()).collect(),
                hierarchy_type: HierarchyType::Temporal,
                description: "Time-based hierarchy for drill-down analysis".to_string(),
            });
        }

        // Look for categorical hierarchies
        let cat_cols: Vec<(usize, &str)> = columns
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let lower = c.name.to_lowercase();
                lower.contains("category") || lower.contains("type") || lower.contains("class")
            })
            .map(|(i, c)| (i, c.name.as_str()))
            .collect();

        if cat_cols.len() >= 2 {
            // Check if one is subset of another (hierarchical)
            for i in 0..cat_cols.len() {
                for j in (i + 1)..cat_cols.len() {
                    if self.is_hierarchical(data, cat_cols[i].0, cat_cols[j].0) {
                        hierarchies.push(GroupingHierarchy {
                            levels: vec![
                                cat_cols[i].1.to_string(),
                                cat_cols[j].1.to_string(),
                            ],
                            hierarchy_type: HierarchyType::Categorical,
                            description: format!(
                                "Drill from {} down to {}",
                                cat_cols[i].1, cat_cols[j].1
                            ),
                        });
                    }
                }
            }
        }

        hierarchies
    }

    fn is_hierarchical(&self, data: &[Vec<String>], col_a: usize, col_b: usize) -> bool {
        // Check if values in col_a consistently map to subset of col_b
        let mut a_to_b: HashMap<String, HashSet<String>> = HashMap::new();

        for row in data.iter().take(1000) {
            if let (Some(val_a), Some(val_b)) = (row.get(col_a), row.get(col_b)) {
                if !val_a.is_empty() && !val_b.is_empty() {
                    a_to_b
                        .entry(val_a.clone())
                        .or_insert_with(HashSet::new)
                        .insert(val_b.clone());
                }
            }
        }

        // If each A value has multiple B values, it's hierarchical
        let avg_b_per_a = a_to_b.values().map(|s| s.len()).sum::<usize>() as f64
            / a_to_b.len() as f64;

        avg_b_per_a > 1.5
    }

    fn suggest_analyses(
        &self,
        dimensions: &[GroupingDimension],
        columns: &[Column],
    ) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Find numeric columns for aggregation
        let numeric_cols: Vec<&str> = columns
            .iter()
            .filter(|c| {
                let lower = c.name.to_lowercase();
                lower.contains("amount")
                    || lower.contains("price")
                    || lower.contains("quantity")
                    || lower.contains("count")
                    || lower.contains("total")
                    || lower.contains("value")
            })
            .map(|c| c.name.as_str())
            .collect();

        // Suggest analyses based on dimensions
        for dim in dimensions {
            match dim.dimension_type {
                DimensionType::Temporal => {
                    suggestions.push(format!(
                        "📊 Group by {}: Analyze trends over time",
                        dim.column_name
                    ));
                    if !numeric_cols.is_empty() {
                        suggestions.push(format!(
                            "   └─ Sum/Average {} per {}",
                            numeric_cols[0], dim.column_name
                        ));
                    }
                }
                DimensionType::Identifier => {
                    suggestions.push(format!(
                        "👤 Group by {}: Analyze per customer/user/product",
                        dim.column_name
                    ));
                    suggestions.push(format!(
                        "   └─ Count records per {} (frequency analysis)",
                        dim.column_name
                    ));
                }
                DimensionType::Categorical => {
                    suggestions.push(format!(
                        "📂 Group by {}: Compare categories",
                        dim.column_name
                    ));
                    if !numeric_cols.is_empty() {
                        suggestions.push(format!(
                            "   └─ Average {} by {}",
                            numeric_cols[0], dim.column_name
                        ));
                    }
                }
                DimensionType::Geographic => {
                    suggestions.push(format!(
                        "🗺️  Group by {}: Regional analysis",
                        dim.column_name
                    ));
                }
                _ => {}
            }
        }

        // Suggest multi-dimensional analysis
        if dimensions.len() >= 2 {
            suggestions.push(format!(""));
            suggestions.push(format!("🔀 Multi-dimensional analysis:"));
            suggestions.push(format!(
                "   └─ Group by {} and {}",
                dimensions[0].column_name, dimensions[1].column_name
            ));
        }

        suggestions
    }
}
