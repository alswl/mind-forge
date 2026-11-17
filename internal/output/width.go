// Adapted from unicode-width 0.1.14, Copyright 2012-2022 The Rust Project Developers.
// Licensed under MIT or Apache-2.0; see third_party/unicode-width.
package output

import "sort"

func widthLookup(char rune) widthRange {
	index := sort.Search(len(widthRanges), func(i int) bool { return widthRanges[i].last >= char })
	if index == len(widthRanges) {
		return widthRange{width: 1}
	}
	return widthRanges[index]
}

func DisplayWidth(text string) int {
	chars := []rune(text)
	total, info := 0, widthDefault
	for index := len(chars) - 1; index >= 0; index-- {
		add, next := widthInString(chars[index], info)
		total += add
		info = next
	}
	return total
}

func widthInString(char rune, next widthInfo) (int, widthInfo) {
	properties := widthLookup(char)
	if next&0x8000 != 0 {
		if properties.flags&1 != 0 {
			if next&0xb000 == 0x9000 {
				return 0, widthEmojiPresentation
			}
			return 2, widthEmojiPresentation
		}
		if next&0x2000 != 0 {
			next &= 0x7fff
		} else {
			next = widthDefault
		}
	}
	if char <= 0xa0 {
		if char == '\n' {
			return 0, widthLineFeed
		}
		if char == '\r' && next == widthLineFeed {
			return 0, widthDefault
		}
		return 1, widthDefault
	}
	if next != widthDefault {
		if char == 0xfe0f {
			if next&0x2000 != 0 || next&0x9000 == 0x1000 {
				return 0, next | 0x8000
			}
			return 0, widthVariationSelector16
		}
		if char == 0xfe0e {
			if next&0x2000 != 0 {
				return 0, next | 0x4000
			}
			return 0, widthVariationSelector15
		}
		if next&0x4000 != 0 {
			if properties.flags&2 != 0 {
				return 1, widthDefault
			}
			next &= 0xbfff
		}
		if next&0x0800 != 0 {
			if char == 0x200d {
				return 0, next | 0x0400
			}
			if properties.flags&8 != 0 {
				return 0, next
			}
		}
		regional := char >= 0x1f1e6 && char <= 0x1f1ff
		consonant := char >= 0x2d31 && char <= 0x2d65 || char == 0x2d6f
		letterTag := char >= 0xe0061 && char <= 0xe007a
		digitTag := char >= 0xe0030 && char <= 0xe0039
		switch {
		case next == widthJoiningGroupAlef && (char == 0x644 || char >= 0x6b5 && char <= 0x6b8 || char == 0x76a || char == 0x8a6 || char == 0x8c7):
			return 0, widthDefault
		case next == widthJoiningGroupAlef && properties.flags&16 != 0:
			return 0, widthJoiningGroupAlef
		case next == widthZwjHebrewLetterLamed && char == 0x5d0:
			return 0, widthDefault
		case next == widthKhmerCoengEligibleLetter && char == 0x17d2:
			return -1, widthDefault
		case next == widthZwjBugineseLetterYa && char == 0x1a17:
			return 0, widthBugineseVowelSignIZwjLetterYa
		case next == widthBugineseVowelSignIZwjLetterYa && char == 0x1a15:
			return 0, widthDefault
		case (next == widthTifinaghConsonant || next == widthZwjTifinaghConsonant) && char == 0x2d7f:
			return 1, widthTifinaghJoinerConsonant
		case next == widthZwjTifinaghConsonant && consonant:
			return 0, widthDefault
		case next == widthTifinaghJoinerConsonant && consonant:
			return -1, widthDefault
		case next == widthLisuToneLetterMyaNaJeu && char >= 0xa4f8 && char <= 0xa4fb:
			return 0, widthDefault
		case next == widthZwjOldTurkicLetterOrkhonI && char == 0x10c32:
			return 0, widthDefault
		case next == widthEmojiModifier && properties.flags&4 != 0:
			return 0, widthEmojiPresentation
		case (next == widthRegionalIndicator || next == widthSeveralRegionalIndicator) && regional:
			return 1, widthSeveralRegionalIndicator
		case (next == widthEmojiPresentation || next == widthSeveralRegionalIndicator || next == widthEvenRegionalIndicatorZwjPresentation || next == widthOddRegionalIndicatorZwjPresentation || next == widthEmojiModifier) && char == 0x200d:
			return 0, widthZwjEmojiPresentation
		case next == widthZwjEmojiPresentation && char == 0x20e3:
			return 0, widthKeycapZwjEmojiPresentation
		case next == widthVs16ZwjEmojiPresentation && properties.flags&1 != 0:
			return 0, widthEmojiPresentation
		case next == widthVs16KeycapZwjEmojiPresentation && (char >= '0' && char <= '9' || char == '#' || char == '*'):
			return 0, widthEmojiPresentation
		case next == widthZwjEmojiPresentation && regional:
			return 1, widthRegionalIndicatorZwjPresentation
		case (next == widthRegionalIndicatorZwjPresentation || next == widthOddRegionalIndicatorZwjPresentation) && regional:
			return -1, widthEvenRegionalIndicatorZwjPresentation
		case next == widthEvenRegionalIndicatorZwjPresentation && regional:
			return 3, widthOddRegionalIndicatorZwjPresentation
		case next == widthZwjEmojiPresentation && char >= 0x1f3fb && char <= 0x1f3ff:
			return 0, widthEmojiModifier
		case next == widthZwjEmojiPresentation && char == 0xe007f:
			return 0, widthTagEndZwjEmojiPresentation
		case next == widthTagEndZwjEmojiPresentation && letterTag:
			return 0, widthTagA1EndZwjEmojiPresentation
		case next >= widthTagA1EndZwjEmojiPresentation && next <= widthTagA5EndZwjEmojiPresentation && letterTag:
			return 0, next + 1
		case (next == widthTagEndZwjEmojiPresentation || next >= widthTagA1EndZwjEmojiPresentation && next <= widthTagA4EndZwjEmojiPresentation) && digitTag:
			return 0, widthTagD1EndZwjEmojiPresentation
		case next == widthTagD1EndZwjEmojiPresentation && digitTag:
			return 0, widthTagD2EndZwjEmojiPresentation
		case next == widthTagD2EndZwjEmojiPresentation && digitTag:
			return 0, widthTagD3EndZwjEmojiPresentation
		case (next >= widthTagA3EndZwjEmojiPresentation && next <= widthTagA6EndZwjEmojiPresentation || next == widthTagD3EndZwjEmojiPresentation) && char == 0x1f3f4:
			return 0, widthEmojiPresentation
		case next == widthZwjEmojiPresentation && properties.info == widthEmojiPresentation:
			return 0, widthEmojiPresentation
		}
	}
	return properties.width, properties.info
}

type widthInfo uint16

const (
	widthDefault                              widthInfo = 0
	widthLineFeed                             widthInfo = 0b0000000000000001
	widthEmojiModifier                        widthInfo = 0b0000000000000010
	widthRegionalIndicator                    widthInfo = 0b0000000000000011
	widthSeveralRegionalIndicator             widthInfo = 0b0000000000000100
	widthEmojiPresentation                    widthInfo = 0b0000000000000101
	widthZwjEmojiPresentation                 widthInfo = 0b0001000000000110
	widthVs16ZwjEmojiPresentation             widthInfo = 0b1001000000000110
	widthKeycapZwjEmojiPresentation           widthInfo = 0b0001000000000111
	widthVs16KeycapZwjEmojiPresentation       widthInfo = 0b1001000000000111
	widthRegionalIndicatorZwjPresentation     widthInfo = 0b0000000000001001
	widthEvenRegionalIndicatorZwjPresentation widthInfo = 0b0000000000001010
	widthOddRegionalIndicatorZwjPresentation  widthInfo = 0b0000000000001011
	widthTagEndZwjEmojiPresentation           widthInfo = 0b0000000000010000
	widthTagD1EndZwjEmojiPresentation         widthInfo = 0b0000000000010001
	widthTagD2EndZwjEmojiPresentation         widthInfo = 0b0000000000010010
	widthTagD3EndZwjEmojiPresentation         widthInfo = 0b0000000000010011
	widthTagA1EndZwjEmojiPresentation         widthInfo = 0b0000000000011001
	widthTagA2EndZwjEmojiPresentation         widthInfo = 0b0000000000011010
	widthTagA3EndZwjEmojiPresentation         widthInfo = 0b0000000000011011
	widthTagA4EndZwjEmojiPresentation         widthInfo = 0b0000000000011100
	widthTagA5EndZwjEmojiPresentation         widthInfo = 0b0000000000011101
	widthTagA6EndZwjEmojiPresentation         widthInfo = 0b0000000000011110
	widthVariationSelector15                  widthInfo = 0b0100000000000000
	widthVariationSelector16                  widthInfo = 0b1000000000000000
	widthJoiningGroupAlef                     widthInfo = 0b0011000011111111
	widthCombiningLongSolidusOverlay          widthInfo = 0b0011110011111111
	widthSolidusOverlayAlef                   widthInfo = 0b0011100011111111
	widthHebrewLetterLamed                    widthInfo = 0b0011100000000000
	widthZwjHebrewLetterLamed                 widthInfo = 0b0011110000000000
	widthBugineseLetterYa                     widthInfo = 0b0011100000000001
	widthZwjBugineseLetterYa                  widthInfo = 0b0011110000000001
	widthBugineseVowelSignIZwjLetterYa        widthInfo = 0b0011110000000010
	widthTifinaghConsonant                    widthInfo = 0b0011100000000011
	widthZwjTifinaghConsonant                 widthInfo = 0b0011110000000011
	widthTifinaghJoinerConsonant              widthInfo = 0b0011110000000100
	widthLisuToneLetterMyaNaJeu               widthInfo = 0b0011110000000101
	widthOldTurkicLetterOrkhonI               widthInfo = 0b0011100000000110
	widthZwjOldTurkicLetterOrkhonI            widthInfo = 0b0011110000000110
	widthKhmerCoengEligibleLetter             widthInfo = 0b0011110000000111
)
