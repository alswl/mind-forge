package report

import (
	"bytes"
	"encoding/xml"
	"errors"
	"fmt"
	"io"
	"regexp"
	"strconv"
	"strings"
)

const svgNamespace = "http://www.w3.org/2000/svg"

var svgID = regexp.MustCompile(`^[a-zA-Z0-9_.:-]+$`)
var localPaint = regexp.MustCompile(`^url\(#([a-zA-Z0-9_.:-]+)\)$`)
var plainPaint = regexp.MustCompile(`^(#[a-fA-F0-9]{3,8}|[a-zA-Z]+|rgba?\([0-9., %]+\))$`)

func member(list, value string) bool { return strings.Contains(" "+list+" ", " "+value+" ") }

const svgElements = "svg g defs symbol use path rect circle ellipse line polyline polygon text tspan title desc clipPath mask linearGradient radialGradient stop pattern marker"
const svgAttributes = "x y x1 x2 y1 y2 width height viewBox preserveAspectRatio d points rx ry cx cy r dx dy transform class role tabindex font-family font-size font-weight text-anchor dominant-baseline alignment-baseline opacity fill-opacity fill-rule clip-rule stroke-width stroke-opacity stroke-dasharray stroke-dashoffset stroke-linejoin stroke-linecap vector-effect shape-rendering overflow offset gradientUnits gradientTransform spreadMethod fx fy stop-opacity clipPathUnits patternUnits patternContentUnits patternTransform maskUnits markerWidth markerHeight refX refY orient"

type svgIDs struct {
	ids, refs map[string]bool
	prefix    string
}

func (s svgIDs) attribute(a xml.Attr) (xml.Attr, error) {
	k, v := a.Name.Local, a.Value
	if a.Name.Space != "" {
		return a, fmt.Errorf("unsupported attribute namespace on %s", k)
	}
	switch {
	case k == "id":
		if !svgID.MatchString(v) || s.ids[v] {
			return a, fmt.Errorf("invalid or duplicate SVG id %q", v)
		}
		s.ids[v] = true
		a.Value = s.prefix + v
	case k == "href":
		id, ok := strings.CutPrefix(v, "#")
		if !ok || !svgID.MatchString(id) {
			return a, fmt.Errorf("expected local href")
		}
		s.refs[id] = true
		a.Value = "#" + s.prefix + id
	case member("fill stroke stop-color color clip-path mask marker-start marker-mid marker-end", k):
		if m := localPaint.FindStringSubmatch(v); m != nil {
			s.refs[m[1]] = true
			a.Value = "url(#" + s.prefix + m[1] + ")"
		} else if !plainPaint.MatchString(v) {
			return a, fmt.Errorf("unsupported %s value %q", k, v)
		}
	case member("aria-labelledby aria-describedby", k):
		ids := strings.Fields(v)
		for i, id := range ids {
			s.refs[id] = true
			ids[i] = s.prefix + id
		}
		a.Value = strings.Join(ids, " ")
	case member(svgAttributes, k) || k == "aria-label" || k == "aria-hidden":
	default:
		return a, fmt.Errorf("unsupported SVG attribute %s", k)
	}
	return a, nil
}

func svgAttr(name, value string) xml.Attr { return xml.Attr{Name: xml.Name{Local: name}, Value: value} }
func viewport(width, height int) []xml.Attr {
	return []xml.Attr{svgAttr("width", strconv.Itoa(width)), svgAttr("height", strconv.Itoa(height)), svgAttr("viewBox", fmt.Sprintf("0 0 %d %d", width, height))}
}

// Each immediate child is one placement. IDs and references are local to it.
func normalizeSVG(raw []byte, f Fragment) ([]byte, error) {
	d := xml.NewDecoder(bytes.NewReader(raw))
	var out bytes.Buffer
	e := xml.NewEncoder(&out)
	depth, count := 0, 0
	rootSeen := false
	var scope svgIDs
	for {
		tok, err := d.Token()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return nil, err
		}
		switch t := tok.(type) {
		case xml.StartElement:
			if t.Name.Space != svgNamespace || !member(svgElements, t.Name.Local) {
				if count > 0 && depth >= 2 {
					return nil, fmt.Errorf("%s: unsupported SVG element %s", f.Components[count-1].Entry, t.Name.Local)
				}
				return nil, fmt.Errorf("unsupported SVG element %s", t.Name.Local)
			}
			if depth == 0 {
				if rootSeen || t.Name.Local != "svg" {
					return nil, fmt.Errorf("expected one svg root")
				}
				rootSeen = true
				for _, a := range t.Attr {
					if a.Name.Space != "" || !member("xmlns width height viewBox", a.Name.Local) {
						return nil, fmt.Errorf("unsupported canvas attribute %s", a.Name.Local)
					}
				}
				t.Attr = viewport(f.Width, f.Height)
				t.Attr = append(t.Attr, svgAttr("xmlns", svgNamespace))
			} else {
				if depth == 1 {
					if count >= len(f.Components) || t.Name.Local != "svg" {
						return nil, fmt.Errorf("unexpected component viewport")
					}
					count++
					scope = svgIDs{map[string]bool{}, map[string]bool{}, fmt.Sprintf("c%d-", count)}
				}
				attrs := make([]xml.Attr, 0, len(t.Attr))
				seen := map[string]bool{}
				for _, a := range t.Attr {
					if a.Name.Local == "xmlns" && a.Name.Space == "" && a.Value == svgNamespace {
						continue
					}
					if seen[a.Name.Local] {
						return nil, fmt.Errorf("duplicate attribute %s", a.Name.Local)
					}
					seen[a.Name.Local] = true
					a, err = scope.attribute(a)
					if err != nil {
						return nil, fmt.Errorf("%s: %w", f.Components[count-1].Entry, err)
					}
					attrs = append(attrs, a)
				}
				t.Attr = attrs
				if depth == 1 {
					p := f.Components[count-1]
					t.Attr = append(viewport(p.Width, p.Height), svgAttr("x", strconv.Itoa(p.X)), svgAttr("y", strconv.Itoa(p.Y)))
				}
			}
			t.Name.Space = ""
			tok = t
			depth++
		case xml.EndElement:
			depth--
			if depth == 1 {
				for ref := range scope.refs {
					if !scope.ids[ref] {
						return nil, fmt.Errorf("%s: unresolved SVG reference %q", f.Components[count-1].Entry, ref)
					}
				}
			}
			t.Name.Space = ""
			tok = t
		case xml.CharData:
			if depth < 2 && strings.TrimSpace(string(t)) != "" {
				return nil, fmt.Errorf("unexpected canvas text")
			}
		case xml.Comment:
			continue
		default:
			return nil, fmt.Errorf("unsupported XML directive")
		}
		if err = e.EncodeToken(tok); err != nil {
			return nil, err
		}
	}
	if !rootSeen || count != len(f.Components) {
		return nil, fmt.Errorf("missing component viewport")
	}
	if err := e.Flush(); err != nil {
		return nil, err
	}
	return out.Bytes(), nil
}
