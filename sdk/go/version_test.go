package extension

import "testing"

func TestCaretRule(t *testing.T) {
	tests := []struct {
		builtFor Version
		host     Version
		accepts  bool
	}{
		{Version{1, 2, 3}, Version{1, 2, 3}, true},
		{Version{1, 2, 3}, Version{1, 9, 0}, true},
		{Version{1, 2, 3}, Version{1, 2, 2}, false},
		{Version{1, 2, 3}, Version{2, 0, 0}, false},
		{Version{0, 1, 2}, Version{0, 1, 9}, true},
		{Version{0, 1, 2}, Version{0, 1, 1}, false},
		{Version{0, 1, 2}, Version{0, 2, 0}, false},
		{Version{0, 0, 3}, Version{0, 0, 3}, true},
		{Version{0, 0, 3}, Version{0, 0, 4}, false},
	}
	for _, test := range tests {
		if got := test.builtFor.accepts(test.host); got != test.accepts {
			t.Errorf("^%s accepts %s: got %t, want %t", test.builtFor, test.host, got, test.accepts)
		}
	}
}
