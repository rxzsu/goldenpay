package goldenpay

import "time"

// OfferGroup defines a group of offers to manage.
type OfferGroup struct {
	NodeID     int64
	ActiveOnly bool
}

func NewOfferGroup(nodeID int64, activeOnly bool) OfferGroup {
	return OfferGroup{NodeID: nodeID, ActiveOnly: activeOnly}
}

// ScheduleRule defines when to apply an action.
type ScheduleRule struct {
	StartHour int // 0-23
	EndHour   int // 0-23
}

func NewScheduleRule(start, end int) ScheduleRule {
	return ScheduleRule{StartHour: start, EndHour: end}
}

func (r ScheduleRule) IsActive(now time.Time) bool {
	h := now.Hour()
	if r.StartHour == r.EndHour {
		return true
	}
	if r.StartHour < r.EndHour {
		return h >= r.StartHour && h < r.EndHour
	}
	return h >= r.StartHour || h < r.EndHour
}

// ScheduleAction is what to do with offers.
type ScheduleAction int

const (
	ActionActivate   ScheduleAction = iota
	ActionDeactivate
)

func (a ScheduleAction) DesiredActive() bool { return a == ActionActivate }

// ScheduleEntry binds a group, rule, and action.
type ScheduleEntry struct {
	Name   string
	Group  OfferGroup
	Rule   ScheduleRule
	Action ScheduleAction
}

func NewScheduleEntry(name string, group OfferGroup, rule ScheduleRule, action ScheduleAction) ScheduleEntry {
	return ScheduleEntry{Name: name, Group: group, Rule: rule, Action: action}
}

// OfferScheduler evaluates entries and reports transitions.
type OfferScheduler struct {
	entries    []ScheduleEntry
	lastStates map[string]bool
}

func NewOfferScheduler(entries []ScheduleEntry) *OfferScheduler {
	return &OfferScheduler{
		entries:    entries,
		lastStates: make(map[string]bool),
	}
}

type Transition struct {
	Entry      *ScheduleEntry
	ShouldBeActive bool
}

func (s *OfferScheduler) Poll(now time.Time) []Transition {
	var transitions []Transition
	for i := range s.entries {
		e := &s.entries[i]
		desired := e.Action.DesiredActive()
		shouldBe := desired
		if !e.Rule.IsActive(now) {
			shouldBe = !desired
		}

		last, ok := s.lastStates[e.Name]
		if !ok || last != shouldBe {
			s.lastStates[e.Name] = shouldBe
			transitions = append(transitions, Transition{Entry: e, ShouldBeActive: shouldBe})
		}
	}
	return transitions
}
