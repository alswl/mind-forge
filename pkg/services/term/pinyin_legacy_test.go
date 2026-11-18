package term

import (
	"reflect"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
)

func TestLegacyPinyinCJKRuns(t *testing.T) {
	rule := func(name, original, pinyin string) model.Term {
		t := termRule(name, original, "Synthetic", "pinyin", "standalone")
		t.Corrections[0].Pinyin = &pinyin
		return t
	}
	for _, test := range []struct {
		name, content string
		rules         []model.Term
		want          []string
	}{
		{"cjk_runs_basic", "hello 你好 world 测试", []model.Term{rule("first", "甲乙", "ni-hao"), rule("second", "甲丙", "ce-shi"), rule("cross", "甲丁", "hao-ce")}, []string{"你好", "测试"}},
		{"cjk_runs_no_cjk", "hello world", []model.Term{rule("first", "甲乙", "ni-hao")}, []string{}},
		{"cjk_runs_skip_exempt_bytes", "前 `你好` 后", []model.Term{rule("left", "甲", "qian"), rule("right", "乙", "hou"), rule("masked", "丙丁", "ni-hao"), rule("cross", "丁戊", "qian-hou")}, []string{"前", "后"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			r, err := Scan(test.content, "synthetic.md", test.rules, Selection{})
			if err != nil {
				t.Fatal(err)
			}
			got := []string{}
			for _, f := range r.Findings {
				got = append(got, f.Original)
				if f.MatchKind != "pinyin" || f.FixKind != "suggested" || f.Boundary != "loose" || f.BoundaryMode != "loose" || f.Selection != "suggested_disabled" {
					t.Fatalf("phonetic rule entered literal matching policy: %+v", f)
				}
			}
			if !reflect.DeepEqual(got, test.want) || len(r.Fixes) != 0 {
				t.Fatalf("CJK run findings = %q, want %q; fixes = %+v", got, test.want, r.Fixes)
			}
		})
	}
}
