# only for test

.PHONY: test
.PHONY: create

BROWSER := google-chrome-stable

test:
	RUSTFLAGS='-C debug-assertions' cargo tarpaulin -p tests --out Xml --release --timeout 600
	pycobertura show --format html --output cobertura.html cobertura.xml
	rm cobertura.xml
	$(BROWSER) cobertura.html

create:
	 cargo test -p tests create --release -- --ignored
	 mv tests/orderDB .
	 mv tests/orderDB.lob .
	 cp orderDB orderDB1
	 cp orderDB.lob orderDB.lob1

# rm orderDB; rm orderDB.lob; cp orderDB1 orderDB && cp orderDB.lob1 orderDB.lob