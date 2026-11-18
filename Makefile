PYTHON ?= python3
REMOTE ?= origin

.PHONY: package release

# Build and smoke-test a local archive without publishing.
package:
	$(PYTHON) scripts/release/package.py $(PACKAGE_FLAGS)

# Push the checked-out channel branch. GitHub Actions owns publication.
release:
	@case "$(CHANNEL)" in main|dev) ;; *) echo 'Choose CHANNEL=main or CHANNEL=dev' >&2; exit 2 ;; esac
	@test "$$(git branch --show-current)" = "$(CHANNEL)" || { echo 'Check out the requested channel branch first.' >&2; exit 2; }
	git push "$(REMOTE)" "refs/heads/$(CHANNEL):refs/heads/$(CHANNEL)"
