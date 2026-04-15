/*==============================================================================================================================

   Copyright © 2003 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : "Common"
   Description          : Common Configuration file

   Author               : RTT
   Version              : 1.0
   Creation Date        : 5/27/2003

================================================================================================================================
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
   Notes for Use and Synthesis
==============================================================================================================================*/

`ifndef  A2S_DEFINES
`define  A2S_DEFINES

`ifndef  A2_DEFINES
`include "A2_defines.vh"
`endif

// CONFIGURATION
//`define  COMBO_FPM
`define  A2S_ENABLE_SHIFTS
//`define  A2S_ENABLE_FLOATING_POINT
//`define  A2S_ENABLE_INTEGER_MULTIPLY
//`define  A2S_ENABLE_INTEGER_DIVIDE
`define  A2S_ENABLE_INTERRUPTS
`define  A2S_ENABLE_CONDITION_CODES
`define  A2S_ENABLE_POPULATION_COUNT
`define  A2S_ENABLE_SWAPBYTES
`define  A2S_ENABLE_UNSIGNED_LOWER

`ifdef   A2S_ENABLE_INTEGER_MULTIPLY
`define  A2S_ENABLE_MULDIV
`elsif   A2S_ENABLE_INTEGER_DIVIDE
`define  A2S_ENABLE_MULDIV
`endif

// BASE PARAMETERS
`define  A2S_MACHINE_WIDTH          32
`define  A2S_INSTRUCTION_WIDTH      32
`define  A2S_OPCODE_WIDTH            6
`define  A2S_IMMEDIATE_WIDTH        16
`define  A2S_FUNCTION_WIDTH         16
`define  A2S_DEPTH_RTRN_STACK       32
`define  A2S_DEPTH_DATA_STACK       32
`define  A2S_IO_ADDRESS_WIDTH       13

`define  A2S_NUM_CONDITION_CODES    10
`define  A2S_NUM_EXTN_INTERRUPTS     4
`define  A2S_STATUS_REG_WIDTH        4

`define  ORDER                      `A2S_INSTRUCTION_WIDTH-1 : `A2S_INSTRUCTION_WIDTH-A2S_OPCODE_WIDTH

// Derived values for Interfaces

// Code Memory Field Sizes
`define  A2S_CM_AD                  `A2S_MACHINE_WIDTH
`define  A2S_CM_EN                   1
`define  A2S_CMOSI_WIDTH            `A2S_CM_EN + `A2S_MACHINE_WIDTH
`define  A2S_CM_DR                  `A2S_MACHINE_WIDTH
`define  A2S_CM_AK                   1
`define  A2S_CMISO_WIDTH            `A2S_CM_AK + `A2S_MACHINE_WIDTH

// Code Memory Field ranges
`define  CMOSI_AD                   `A2S_MACHINE_WIDTH - 1 : 0
`define  CMOSI_EN                   `A2S_MACHINE_WIDTH

`define  CMISO_DR                   `A2S_MACHINE_WIDTH - 1 : 0
`define  CMISO_AK                   `A2S_MACHINE_WIDTH

// Data Memory Field Sizes
`define  A2S_DM_AD                  `A2S_MACHINE_WIDTH
`define  A2S_DM_DW                  `A2S_MACHINE_WIDTH
`define  A2S_DM_WR                   1
`define  A2S_DM_EN                   1
`define  A2S_DMOSI_WIDTH            `A2S_DM_EN + `A2S_DM_WR + `A2S_DM_DW + `A2S_DM_AD
`define  A2S_DM_DR                  `A2S_MACHINE_WIDTH
`define  A2S_DM_AK                   1
`define  A2S_DMISO_WIDTH            `A2S_MACHINE_WIDTH+1

// Data Memory Field ranges
`define  DMOSI_EN                   `A2S_DM_AD + `A2S_DM_DW + `A2S_DM_WR + `A2S_DM_EN -1
`define  DMOSI_WR                   `A2S_DM_AD + `A2S_DM_DW + `A2S_DM_WR -1
`define  DMOSI_DW                   `A2S_DM_AD + `A2S_DM_DW - 1  :  `A2S_DM_AD
`define  DMOSI_AD                   `A2S_DM_AD - 1 : 0

`define  DMISO_DR                   `A2S_DM_DR - 1 : 0
`define  DMISO_AK                   `A2S_DM_AK + `A2S_DM_DR -1

// I/O Interface Field Sizes
`define  A2S_IO_WD                  `A2S_MACHINE_WIDTH
`define  A2S_IO_AD                  `A2S_IO_ADDRESS_WIDTH
`define  A2S_IO_CM                    3
`define  A2S_IMOSI_WIDTH            `A2S_IO_WD+`A2S_IO_AD+`A2S_IO_CM
`define  A2S_IO_RD                  `A2S_MACHINE_WIDTH
`define  A2S_IO_AK                    1
`define  A2S_IMISO_WIDTH            `A2S_IO_RD+`A2S_IO_AK

// I/O Interface Field Ranges
`define  IMOSI_CM                   `A2S_IO_AD + `A2S_IO_WD + `A2S_IO_CM -1 : `A2S_IO_AD + `A2S_IO_WD
`define  IMOSI_DA                   `A2S_IO_AD + `A2S_IO_WD -1 : `A2S_IO_AD
`define  IMOSI_AD                   `A2S_IO_AD -1 :  0

`define  IMISO_DO                   `A2S_IO_WD -1 : 0
`define  IMISO_AK                   `A2S_IO_WD

`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
