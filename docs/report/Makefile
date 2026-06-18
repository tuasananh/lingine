# Makefile for building the Lingine Project Report PDF

MAIN = main
TEX_FILES = main.tex title.tex chap1.tex chap2.tex chap3.tex chap4.tex chap5.tex chap6.tex refs.tex

all: pdf

pdf: $(TEX_FILES)
	pdflatex $(MAIN).tex
	pdflatex $(MAIN).tex

clean:
	rm -f *.aux *.log *.out *.toc *.pdf

.PHONY: all pdf clean
