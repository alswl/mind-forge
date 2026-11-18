package lifecycle

import (
	"encoding/json"
	"fmt"
	"os"
	"testing"
)

func TestAdditionalEveryDurableStageRecovers(t *testing.T) {
	raw, err := os.ReadFile("testdata/additional-recovery-plans.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixtures []struct {
		Subject       string `json:"subject"`
		Layout        string `json:"layout"`
		Remove        bool   `json:"remove"`
		PreviewWrites int    `json:"preview_writes"`
		Plan          Plan   `json:"plan"`
	}
	if err := json.Unmarshal(raw, &fixtures); err != nil {
		t.Fatal(err)
	}
	if len(fixtures) != 8 {
		t.Fatal("missing layout/operation/subject fixtures")
	}
	for _, fixture := range fixtures {
		t.Run(fmt.Sprintf("subject=%s/layout=%s/remove=%t", fixture.Subject, fixture.Layout, fixture.Remove), func(t *testing.T) {
			preview, err := fixture.Plan.Preview()
			if err != nil || len(preview) != fixture.PreviewWrites {
				t.Fatalf("component preview stages differ from actual published CLI writes:got%d want%d err%v", len(preview), fixture.PreviewWrites, err)
			}
			assertEveryDurableStageRecovers(t, fixture.Plan)
		})
	}
}
