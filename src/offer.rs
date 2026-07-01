//! Builder for constructing [`OfferEdit`] patches.

use crate::models::OfferEdit;

/// Builder for constructing [`OfferEdit`] patches with a fluent API.
#[derive(Debug, Clone, Default)]
pub struct OfferEditBuilder {
    inner: OfferEdit,
}

impl OfferEditBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the quantity field.
    pub fn quantity(mut self, value: impl Into<String>) -> Self {
        self.inner.quantity = Some(value.into());
        self
    }

    /// Sets the price field.
    pub fn price(mut self, value: impl Into<String>) -> Self {
        self.inner.price = Some(value.into());
        self
    }

    /// Sets the Russian description.
    pub fn desc_ru(mut self, value: impl Into<String>) -> Self {
        self.inner.desc_ru = Some(value.into());
        self
    }

    /// Sets the English description.
    pub fn desc_en(mut self, value: impl Into<String>) -> Self {
        self.inner.desc_en = Some(value.into());
        self
    }

    /// Sets whether the offer is active.
    #[must_use]
    pub fn active(mut self, value: bool) -> Self {
        self.inner.active = Some(value);
        self
    }

    /// Sets whether the offer deactivates after a sale.
    #[must_use]
    pub fn deactivate_after_sale(mut self, value: bool) -> Self {
        self.inner.deactivate_after_sale = Some(value);
        self
    }

    /// Consumes the builder and returns the [`OfferEdit`].
    #[must_use]
    pub fn build(self) -> OfferEdit {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_offer_patch() {
        let edit = OfferEditBuilder::new()
            .quantity("5")
            .price("399")
            .active(true)
            .build();

        assert_eq!(edit.quantity.as_deref(), Some("5"));
        assert_eq!(edit.price.as_deref(), Some("399"));
        assert_eq!(edit.active, Some(true));
    }
}
