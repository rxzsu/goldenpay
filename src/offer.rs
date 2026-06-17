use crate::models::OfferEdit;

#[derive(Debug, Clone, Default)]
pub struct OfferEditBuilder {
    inner: OfferEdit,
}

impl OfferEditBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn quantity(mut self, value: impl Into<String>) -> Self {
        self.inner.quantity = Some(value.into());
        self
    }

    pub fn price(mut self, value: impl Into<String>) -> Self {
        self.inner.price = Some(value.into());
        self
    }

    pub fn desc_ru(mut self, value: impl Into<String>) -> Self {
        self.inner.desc_ru = Some(value.into());
        self
    }

    pub fn desc_en(mut self, value: impl Into<String>) -> Self {
        self.inner.desc_en = Some(value.into());
        self
    }

    #[must_use]
    pub fn active(mut self, value: bool) -> Self {
        self.inner.active = Some(value);
        self
    }

    #[must_use]
    pub fn deactivate_after_sale(mut self, value: bool) -> Self {
        self.inner.deactivate_after_sale = Some(value);
        self
    }

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
