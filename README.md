# Eludris

A free and open source, federated, End-to-end-Encrypted social media platform made in rust that's easy to deploy and configure while striving to be *truly **yours***.

## Deployment

We really recommend and *only* officially support using the provided docker-compose as a quick way to get stuff running, just edit your `Eludris.toml` to suit your needs then run

```sh
docker-compose up
```

Congratulations, you've now successfully deployed your Eludris instance!

## Default Ports

[Oprish](https://github.com/eludris/oprish) (HTTP API): 7159

[Pandemonium](https://github.com/eludris/pandemonium) (WS API/ Gateway): 7160

[Effis](https://github.com/eludris/effis) (File server, CDN and proxy): 7161
