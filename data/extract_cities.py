#!/usr/bin/python3
with open("geonames_cities1000_2017-02-27_02:01.tsv") as r:
    print("population\tname")
    for line in r:
        parts = line.split("\t")
        print("%s\t%s" % (parts[14], parts[1]))
