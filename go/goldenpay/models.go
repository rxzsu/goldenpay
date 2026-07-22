package goldenpay

// UserInfo holds authenticated user metadata.
type UserInfo struct {
	ID        int64  `json:"id"`
	Username  string `json:"username"`
	CSRFToken string `json:"csrf_token"`
	PHPSessID string `json:"phpsessid,omitempty"`
}

// OrderInfo is a compact order from the trade page.
type OrderInfo struct {
	ID              string      `json:"id"`
	BuyerUsername   string      `json:"buyer_username"`
	BuyerID         int64       `json:"buyer_id"`
	ChatID          string      `json:"chat_id"`
	Description     string      `json:"description"`
	SubcategoryName string      `json:"subcategory_name"`
	Amount          int32       `json:"amount"`
	Status          OrderStatus `json:"status"`
}

type OrderStatus string

const (
	OrderPaid    OrderStatus = "paid"
	OrderClosed  OrderStatus = "closed"
	OrderRefunded OrderStatus = "refunded"
)

// OrderPage is a detailed order with secrets and review.
type OrderPage struct {
	ID              string    `json:"id"`
	Status          OrderStatus `json:"status"`
	Amount          int32       `json:"amount"`
	Sum             float64     `json:"sum"`
	Currency        string      `json:"currency"`
	BuyerID         int64       `json:"buyer_id"`
	BuyerUsername   string      `json:"buyer_username"`
	ChatID          string      `json:"chat_id"`
	ShortDesc       string      `json:"short_description,omitempty"`
	FullDesc        string      `json:"full_description,omitempty"`
	SubcategoryName string      `json:"subcategory_name,omitempty"`
	Secrets         []string    `json:"secrets"`
	Params          [][2]string `json:"params"`
	Review          *Review     `json:"review,omitempty"`
	RawHTML         string      `json:"raw_html"`
}

type Review struct {
	Stars int    `json:"stars"`
	Text  string `json:"text,omitempty"`
}

// ChatMessage represents a single chat message.
type ChatMessage struct {
	ID       int64  `json:"id"`
	ChatID   string `json:"chat_id"`
	AuthorID int64  `json:"author_id"`
	Text     string `json:"text,omitempty"`
}

// Offer is a seller's own offer on the trade page.
type Offer struct {
	ID          int64   `json:"id"`
	NodeID      int64   `json:"node_id"`
	Description string  `json:"description"`
	Price       float64 `json:"price"`
	Currency    string  `json:"currency"`
	Active      bool    `json:"active"`
}

// OfferEdit patches an offer.
type OfferEdit struct {
	Quantity            *string `json:"quantity,omitempty"`
	Price               *string `json:"price,omitempty"`
	Active              *bool   `json:"active,omitempty"`
	Deleted             *bool   `json:"deleted,omitempty"`
	DescriptionRU       *string `json:"desc_ru,omitempty"`
	DescriptionEN       *string `json:"desc_en,omitempty"`
	DeactivateAfterSale *bool   `json:"deactivate_after_sale,omitempty"`
}

// PriceCalculation holds price breakdown.
type PriceCalculation struct {
	InputPrice    float64            `json:"input_price"`
	SellerPrice   *float64           `json:"seller_price,omitempty"`
	BuyerPrice    *float64           `json:"buyer_price,omitempty"`
	Commission    *float64           `json:"commission,omitempty"`
	NumericFields map[string]float64 `json:"numeric_fields"`
}

// CategoryNode is a node in the marketplace category tree.
type CategoryNode struct {
	ID              int64              `json:"id"`
	Name            string             `json:"name"`
	SubcategoryType *string            `json:"subcategory_type,omitempty"`
	Children        []CategoryNode     `json:"children"`
}

// BotState for persistence.
type BotState struct {
	SeenOrders   []string          `json:"seen_orders"`
	SeenMessages map[string]int64  `json:"seen_messages"`
}
