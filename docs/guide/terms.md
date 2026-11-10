# Check and correct terms

```bash
mf term new "Release Note" --project notes
mf term lint --project notes
mf term fix --project notes --ad-hoc 'listnode=>Release Note' --yes
```

By default, lint and fix leave blockquotes, inline code and `「verbatim spans」`
alone. Pass `--include-quotes` when quoted text should be corrected too. See
the [correction reference](../manual/term-corrections.md).
