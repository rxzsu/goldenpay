package goldenpay

import (
	"context"
	"log"
	"time"
)

// GoldenPayEvent emitted by the bot.
type GoldenPayEvent struct {
	NewOrder   *OrderInfo
	NewMessage *ChatMessage
}

// BotOptions configures the bot.
type BotOptions struct {
	IgnoreOwnMessages       bool
	AutoWelcomeMessage      string
	SleepScheduleStart      int
	SleepScheduleEnd        int
	SleepNodeOffers         [][2]int64 // (node_id, offer_id) pairs
}

func DefaultBotOptions() BotOptions {
	return BotOptions{IgnoreOwnMessages: true}
}

// GoldenPayBot polls for new orders and messages.
type GoldenPayBot struct {
	session  *GoldenPaySession
	options  BotOptions
	store    StateStore
	seen     *EventStream
}

type EventStream struct {
	SeenOrders   map[string]struct{}
	SeenMessages map[string]int64
}

func NewBot(session *GoldenPaySession) *GoldenPayBot {
	return &GoldenPayBot{
		session: session,
		options: DefaultBotOptions(),
		store:   NewMemoryStateStore(),
		seen: &EventStream{
			SeenOrders:   make(map[string]struct{}),
			SeenMessages: make(map[string]int64),
		},
	}
}

func (b *GoldenPayBot) WithStore(store StateStore) *GoldenPayBot {
	b.store = store
	return b
}

func (b *GoldenPayBot) WithOptions(opts BotOptions) *GoldenPayBot {
	b.options = opts
	return b
}

func (b *GoldenPayBot) LoadState() error {
	state, err := b.store.Load()
	if err != nil {
		return err
	}
	for _, id := range state.SeenOrders {
		b.seen.SeenOrders[id] = struct{}{}
	}
	for k, v := range state.SeenMessages {
		b.seen.SeenMessages[k] = v
	}
	return nil
}

func (b *GoldenPayBot) SaveState() error {
	state := &BotState{
		SeenMessages: b.seen.SeenMessages,
	}
	for id := range b.seen.SeenOrders {
		state.SeenOrders = append(state.SeenOrders, id)
	}
	return b.store.Save(state)
}

func (b *GoldenPayBot) Run(ctx context.Context, handler func(GoldenPayEvent) error) error {
	interval := b.session.Config().PollInterval
	sleepState := -1 // -1 unknown, 0 awake, 1 sleeping

	for {
		select {
		case <-ctx.Done():
			return nil
		default:
		}

		// Handle sleep schedule if configured
		if b.options.SleepNodeOffers != nil {
			now := time.Now()
			h := now.Hour()
			r := b.options.SleepScheduleStart <= h && h < b.options.SleepScheduleEnd
			if b.options.SleepScheduleStart > b.options.SleepScheduleEnd {
				r = h >= b.options.SleepScheduleStart || h < b.options.SleepScheduleEnd
			}
			shouldSleep := r
			targetSleep := 0
			if shouldSleep {
				targetSleep = 1
			}

			if sleepState != targetSleep {
				sleepState = targetSleep
				active := !shouldSleep
				for _, pair := range b.options.SleepNodeOffers {
					_, err := b.session.EditOffer(pair[0], pair[1], &OfferEdit{Active: &active})
					if err != nil {
						log.Printf("sleep schedule: failed to update offer %d/%d: %v", pair[0], pair[1], err)
					}
				}
			}
		}

		// Poll orders & messages (simplified)
		if err := b.pollOnce(handler); err != nil {
			log.Printf("poll error: %v", err)
		}

		time.Sleep(interval)
	}
}

func (b *GoldenPayBot) pollOnce(handler func(GoldenPayEvent) error) error {
	_ = handler
	// Placeholder: order fetching and event emission would go here
	return nil
}

// Stub methods that need HTTP integration
func (s *GoldenPaySession) EditOffer(nodeID, offerID int64, edit *OfferEdit) (*OfferSaveResponse, error) {
	_ = nodeID
	_ = offerID
	_ = edit
	return nil, nil
}

type OfferSaveResponse struct {
	Success bool
	Error   string
}
