package goldenpay

import "fmt"

// Urls builds FunPay API URLs.
type Urls struct {
	base string
}

func NewUrls(base string) *Urls {
	return &Urls{base: base}
}

func (u *Urls) Base() string              { return u.base }
func (u *Urls) Home() string              { return fmt.Sprintf("%s/", u.base) }
func (u *Urls) Runner() string            { return fmt.Sprintf("%s/runner/", u.base) }
func (u *Urls) OrdersTrade() string       { return fmt.Sprintf("%s/orders/trade", u.base) }
func (u *Urls) OrderPage(id string) string { return fmt.Sprintf("%s/orders/%s/", u.base, id) }
func (u *Urls) LotsCalc() string          { return fmt.Sprintf("%s/lots/calc", u.base) }
func (u *Urls) LotsHome() string          { return fmt.Sprintf("%s/lots/", u.base) }
func (u *Urls) LotsPage(nodeID int64) string      { return fmt.Sprintf("%s/lots/%d/", u.base, nodeID) }
func (u *Urls) LotsTrade(nodeID int64) string     { return fmt.Sprintf("%s/lots/%d/trade", u.base, nodeID) }
func (u *Urls) OfferEdit(nodeID, offerID int64) string {
	return fmt.Sprintf("%s/lots/offerEdit?node=%d&offer=%d", u.base, nodeID, offerID)
}
func (u *Urls) OfferSave() string { return fmt.Sprintf("%s/lots/offerSave", u.base) }
