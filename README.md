Cloud diagram: ![test-task drawio](https://github.com/litvaOo/revolut-test-task/assets/14944792/2ed97d33-e7a9-4f4e-8101-819dccaef652)

## Deploy to cloud: 
  1. `cd infra && terraform apply -auto-approve`
  2. Trigger Main CI deployment

## Run locally
`docker compose up`
Access on localhost:3000
Compose automatically mounts `service` directory into container, live-reload is enabled by default
