#[derive(Debug, Clone)]
pub struct Urls {
    base: String,
}

impl Urls {
    pub fn new(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }

    pub fn base(&self) -> &str {
        self.base.trim_end_matches('/')
    }

    pub fn home(&self) -> String {
        format!("{}/", self.base())
    }

    pub fn runner(&self) -> String {
        format!("{}/runner/", self.base())
    }

    pub fn orders_trade(&self) -> String {
        format!("{}/orders/trade", self.base())
    }

    pub fn order_page(&self, order_id: &str) -> String {
        format!("{}/orders/{order_id}/", self.base())
    }

    pub fn offer_edit(&self, node_id: i64, offer_id: i64) -> String {
        format!(
            "{}/lots/offerEdit?node={node_id}&offer={offer_id}",
            self.base()
        )
    }

    pub fn offer_save(&self) -> String {
        format!("{}/lots/offerSave", self.base())
    }

    pub fn lots_trade(&self, node_id: i64) -> String {
        format!("{}/lots/{node_id}/trade", self.base())
    }

    pub fn lots_page(&self, node_id: i64) -> String {
        format!("{}/lots/{node_id}/", self.base())
    }

    pub fn lots_home(&self) -> String {
        format!("{}/lots/", self.base())
    }

    pub fn lots_calc(&self) -> String {
        format!("{}/lots/calc", self.base())
    }
}
