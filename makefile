# only for test

BROWSER := firefox-nightly

test:
	cargo tarpaulin -p tests --out Xml --release --timeout 600
	pycobertura show --format html --output cobertura.html cobertura.xml
	rm cobertura.xml
	$(BROWSER) cobertura.html