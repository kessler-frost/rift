use crate::search::QueryFilter;

/// Trait to render a filter chip.
pub trait FilterChipRenderer {
    /// Returns how much larger the icon should be than the font size.
    fn icon_size_offset(&self) -> f32;

    /// Returns the margin from the top of the icon of the filter chip.
    fn icon_margin_top(&self) -> f32;
}

impl FilterChipRenderer for QueryFilter {
    fn icon_size_offset(&self) -> f32 {
        match self {
            QueryFilter::NaturalLanguage => 2.,
            _ => 0.,
        }
    }

    fn icon_margin_top(&self) -> f32 {
        match self {
            QueryFilter::Sessions => 2.,
            QueryFilter::Tabs => 2.,
            QueryFilter::NaturalLanguage => 2.,
            _ => 0.,
        }
    }
}
