export type UsageEntry = {
  timestamp: string
  tool: string
  model: string
  input_tokens: number
  output_tokens: number
  cache_read_tokens: number
  cache_write_tokens: number
  cost: number
}

