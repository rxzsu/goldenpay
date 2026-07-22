package goldenpay

import (
	"encoding/json"
	"os"
	"sync"
)

// StateStore persists bot state.
type StateStore interface {
	Load() (*BotState, error)
	Save(state *BotState) error
}

// MemoryStateStore is an in-memory store (non-persistent).
type MemoryStateStore struct {
	mu    sync.Mutex
	state *BotState
}

func NewMemoryStateStore() *MemoryStateStore {
	return &MemoryStateStore{state: &BotState{
		SeenMessages: make(map[string]int64),
	}}
}

func (s *MemoryStateStore) Load() (*BotState, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	cp := *s.state
	cp.SeenMessages = make(map[string]int64)
	for k, v := range s.state.SeenMessages {
		cp.SeenMessages[k] = v
	}
	cp.SeenOrders = append([]string{}, s.state.SeenOrders...)
	return &cp, nil
}

func (s *MemoryStateStore) Save(state *BotState) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.state = state
	return nil
}

// JSONStateStore persists state to a JSON file.
type JSONStateStore struct {
	path string
	mu   sync.Mutex
}

func NewJSONStateStore(path string) *JSONStateStore {
	return &JSONStateStore{path: path}
}

func (s *JSONStateStore) Load() (*BotState, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	data, err := os.ReadFile(s.path)
	if err != nil {
		if os.IsNotExist(err) {
			return &BotState{SeenMessages: make(map[string]int64)}, nil
		}
		return nil, err
	}

	var state BotState
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, err
	}
	if state.SeenMessages == nil {
		state.SeenMessages = make(map[string]int64)
	}
	return &state, nil
}

func (s *JSONStateStore) Save(state *BotState) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(s.path, data, 0644)
}
