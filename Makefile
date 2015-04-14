all: zhtta gash

zhtta: zhtta.rs
	rustc zhtta.rs

gash: gash.rs
	rustc gash.rs

clean :
	$(RM) zhtta gash
    
run: zhtta
	./zhtta

