# P4 integrated final automated gate

Result: **FAIL**

Workflow run: 31288505076
Validated source: e7abf410ce4e77fe9c6f148df087874c1f39d775
Branch: agent/p4-integrated-world-pipeline

## Failure summary

```text
```

## Diagnostic tail

```text
## Static preproduction checks
Architecture validation passed.
Traceback (most recent call last):
  File "/home/runner/work/Prototype-Validation/Prototype-Validation/scripts/validate_workspace.py", line 71, in <module>
    raise SystemExit(main())
                     ^^^^^^
  File "/home/runner/work/Prototype-Validation/Prototype-Validation/scripts/validate_workspace.py", line 63, in main
    assert stages["P4"]["status"] == "integrated-validation-in-progress"
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AssertionError
```
