PA3
===

Starting code for Programming Assignment 3 of cs146a

Includes:

- zhtta.rs: starting code for Zhtta web server.

- gash.rs: reference solution of gash shell (PA2).

- zhtta-test.txt: list of test URLs separated by newline

- zhtta-test-NUL.txt: list of test URLs separated by ASCII NUL

- www/index.shtml: a simple test file


to generate an ASCII NUL separated file by a newline separated run:

tr "\n" "\0" < zhtta-test.txt > zhtta-test-NUL.txt
