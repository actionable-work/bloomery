{pkgs}: {
  optimize = {
    nativeBuildInputs = [pkgs.hello];
    env = {BLOOMERY_OPTIMIZE_FIXTURE = "1";};
    fileset = ./training;
  };
}
