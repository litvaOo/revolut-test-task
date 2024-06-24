terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.55.0"
    }
  }
  backend "s3" {
    bucket = "revolut-test-task"
    key    = "terraform.tfstate"
    region = "us-east-1"
  }

  required_version = ">= 1.8.5"
}

provider "aws" {
  region = "us-east-1"
}

module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "v5.8.1"
  name    = "test-task"

  cidr = "10.0.0.0/16"

  azs                          = ["us-east-1a", "us-east-1b", "us-east-1c"]
  private_subnets              = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
  public_subnets               = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]
  enable_nat_gateway           = true
  database_subnets             = ["10.0.4.0/24", "10.0.5.0/24", "10.0.6.0/24"]
  create_database_subnet_group = true
  create_egress_only_igw       = true
}

resource "aws_security_group" "rds" {
  vpc_id = module.vpc.vpc_id
  ingress {
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = module.vpc.private_subnets_cidr_blocks
  }
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = module.vpc.private_subnets_cidr_blocks
  }
}

module "db" {
  source  = "terraform-aws-modules/rds/aws"
  version = "v6.7.0"

  identifier     = "test-task"
  engine         = "postgres"
  engine_version = "16.3"
  instance_class = "db.t4g.micro"

  db_name  = "testdb"
  username = "testuser"
  port     = 5432

  family = "postgres16"

  db_subnet_group_name = module.vpc.database_subnet_group_name

  create_monitoring_role          = true
  monitoring_interval             = 60
  monitoring_role_name            = "test-task-monitoring-role"
  monitoring_role_use_name_prefix = true

  allocated_storage      = 20
  vpc_security_group_ids = [aws_security_group.rds.id]

  publicly_accessible = false
  storage_encrypted   = true

  manage_master_user_password = true
}

module "ecr" {
  source  = "terraform-aws-modules/ecr/aws"
  version = "v2.2.1"

  repository_name = "test-task"
  repository_type = "private"
  repository_lifecycle_policy = jsonencode({
    rules = [
      {
        rulePriority = 1
        description  = "Expire images older than 14 days"
        selection = {
          tagStatus   = "untagged"
          countType   = "sinceImagePushed"
          countUnit   = "days"
          countNumber = 14
        }
        action = {
          type = "expire"
        }
    }]
  })
}

resource "aws_iam_role" "execution" {
  name = "test-task-execution-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Action = "sts:AssumeRole",
        Effect = "Allow",
        Principal = {
          Service = "ecs-tasks.amazonaws.com"
        }
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "execution" {
  role       = aws_iam_role.execution.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

resource "aws_iam_policy" "execution" {
  name = "test-task-execution-policy"
  policy = jsonencode({
    Version = "2012-10-17",
    Statement = [
      {
        Effect = "Allow",
        Action = [
          "secretsmanager:GetSecretValue",
          "secretsmanager:DescribeSecret"
        ],
        Resource = module.db.db_instance_master_user_secret_arn
      },
      {
        Effect = "Allow",
        Action = [
          "ecr:GetDownloadUrlForLayer",
          "ecr:BatchGetImage",
          "ecr:BatchCheckLayerAvailability"
        ],
        Resource = module.ecr.repository_arn
      }
    ]
  })
}

module "alb" {
  source  = "terraform-aws-modules/alb/aws"
  version = "v9.9.0"

  name    = "test-task"
  vpc_id  = module.vpc.vpc_id
  subnets = module.vpc.public_subnets
  security_group_ingress_rules = {
    http = {
      from_port = 80
      to_port   = 80
      protocol  = "tcp"
      cidr_ipv4 = "0.0.0.0/0"
    }
    https = {
      from_port = 443
      to_port   = 443
      protocol  = "tcp"
      cidr_ipv4 = "0.0.0.0/0"
    }
  }
  security_group_egress_rules = {
    all_traffic = {
      from_port = 0
      to_port   = 0
      protocol  = "-1"
      cidr_ipv4 = "0.0.0.0/0"
    }
  }
  listeners = {
    http = {
      port     = 80
      protocol = "HTTP"
      forward = {
        target_group_key = "instance"
      }
    }
  }

  target_groups = {
    instance = {
      name_prefix       = "test"
      create_attachment = false
      backend_protocol  = "HTTP"
      backend_port      = 80
      target_type       = "ip"
      health_check = {
        path                = "/"
        protocol            = "HTTP"
        matcher             = "200"
        interval            = 30
        timeout             = 5
        healthy_threshold   = 2
        unhealthy_threshold = 2
      }
    }
  }
}

resource "aws_security_group" "ecs" {
  vpc_id = module.vpc.vpc_id
  ingress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = module.vpc.public_subnets_cidr_blocks
  }
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "tcp"
    cidr_blocks = module.vpc.public_subnets_cidr_blocks
  }
  egress {
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = module.vpc.database_subnets_cidr_blocks
  }
}

module "ecs" {
  source  = "terraform-aws-modules/ecs/aws"
  version = "v5.11.2"

  cluster_name = "test-task"

  fargate_capacity_providers = {
    FARGATE = {
      default_capacity_provider_strategy = {
        weight = 50
      }
    }
    FARGATE_SPOT = {
      default_capacity_provider_strategy = {
        weight = 50
      }
    }
  }


  services = {
    test-task = {
      subnet_ids             = module.vpc.private_subnets
      task_exec_iam_role_arn = aws_iam_role.execution.arn
      load_balancer = {
        service = {
          container_name   = "test-task"
          container_port   = 80
          target_group_arn = module.alb.target_groups["instance"].arn
        }
      }
      cpu    = 256
      memory = 512
      container_definitions = {
        test-task = {
          name      = "test-task"
          essential = true
          cpu       = 256
          memory    = 512
          image     = "${module.ecr.repository_url}:latest"
          port_mappings = [{
            name          = "test-task"
            containerPort = 80
            protocol      = "tcp"
          }]
        }
      }
    }
  }
}
