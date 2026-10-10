R=/tmp/cpt_fz; rm -rf $R; mkdir -p $R/src/sub; echo -n a > $R/src/a; chmod 437 $R/src/a; echo -n x > $R/src/sub/c; chmod 400 $R/src/sub/c; cp -R $R/src $R/dst; echo RC=$?; du -a $R/dst; rm -rf $R
