# only for test

.PHONY: test
.PHONY: create

BROWSER := firefox-nightly

test:
	RUSTFLAGS='-C debug-assertions' cargo tarpaulin -p tests --out Xml --release --timeout 600
	pycobertura show --format html --output cobertura.html cobertura.xml
	rm cobertura.xml
	$(BROWSER) cobertura.html

create:
	 RUSTFLAGS='-C debug-assertions' cargo test -p tests create --release -- --ignored
	 mv tests/orderDB .
	 cp orderDB orderDB1