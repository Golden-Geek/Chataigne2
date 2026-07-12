# Phase 0 product manifests

`product_manifest.py` inventories declarations and files in the current product
without treating their existence as proof of working behavior. It produces:

- versioned product-surface and product-file manifests;
- baseline project fixtures from explicit fixture directories such as
  `fixtures/` and `test-samples/`, regardless of file extension;
- the exact recursively discovered Git submodule gitlinks;
- a granular parity row for every discovered item, initially marked `baseline`
  and `pending` with an evidence ID and explicit characterization placeholders;
- JSON Schemas used by both generation and validation.

From the repository root:

```powershell
python tools/migration/product_manifest.py generate
python tools/migration/product_manifest.py check
python tools/migration/product_manifest.py validate
python -m unittest discover -s tools/migration/tests -v
```

`check` regenerates the complete expected document set in memory, validates it,
and compares canonical UTF-8 JSON bytes with the committed files. The output has
no timestamp or working-tree-dependent root commit, so an unchanged source tree
is byte-for-byte reproducible. Gitlink SHAs come from committed index entries,
not whichever submodule branch happens to be checked out.

The file manifest separately reports whether canonical `P50-L1` and `P5-L127`
performance fixtures were actually discovered. Other baseline projects do not
silently satisfy those named requirements.
