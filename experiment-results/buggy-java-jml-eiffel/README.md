This folder contains the test results for the `buggy-java-jml-eiffel` dataset.

The patch is split to avoid the problem of Overleaf not allowing big files (2 MB).

To create the patch file:

``` sh
cat x* > anonymized-buggy-java-jml-eiffel-evaluation.patch
```

To split the patch file:
``` sh
split -b 1M anonymized-buggy-java-jml-eiffel-evaluation.patch
```
