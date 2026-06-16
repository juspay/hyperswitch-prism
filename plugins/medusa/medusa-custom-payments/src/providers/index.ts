import { ModuleProvider, Modules } from "@medusajs/framework/utils"
import HyperswitchPrismService from "./service"

const services = [HyperswitchPrismService]

export default ModuleProvider(Modules.PAYMENT, { services })
